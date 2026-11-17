param(
  [string]$BuildRoot = (Join-Path (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path "desktop-rust/target/windows-cores"),
  [string]$PlayerExe = (Join-Path (Resolve-Path (Join-Path $PSScriptRoot "..")).Path "target/debug/vyntra-desktop.exe"),
  [int]$Jobs = [Math]::Max(1, [Environment]::ProcessorCount - 1)
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$buildRoot = [IO.Path]::GetFullPath($BuildRoot)
$gbBuild = Join-Path $buildRoot "build/gbc"
$dsBuild = Join-Path $buildRoot "build/ds"
$dist = Join-Path $buildRoot "cores"
$mingwC = "x86_64-w64-mingw32-gcc"
$mingwCxx = "x86_64-w64-mingw32-g++"

foreach ($tool in @("cmake", "ninja", $mingwC, $mingwCxx, "python")) {
  if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "Required build tool not found on PATH: $tool" }
}
foreach ($required in @(
  "third_party/mgba/CMakeLists.txt",
  "third_party/dsperate-lr/DSperate/CMakeLists.txt",
  "linux/libretro-ds/CMakeLists.txt"
)) {
  if (-not (Test-Path (Join-Path $repo $required))) { throw "Missing source or submodule: $required" }
}
if (-not (Test-Path -LiteralPath $PlayerExe)) { throw "Build the Windows Desktop executable first: $PlayerExe" }

New-Item -ItemType Directory -Force $gbBuild, $dsBuild, $dist | Out-Null
$common = @(
  "-G", "Ninja",
  "-DCMAKE_BUILD_TYPE=Release",
  "-DCMAKE_C_COMPILER=$mingwC",
  "-DCMAKE_CXX_COMPILER=$mingwCxx"
)

Write-Host "Configuring PokeDaisy mGBA GBC/GBA core..."
& cmake -S (Join-Path $repo "libretro") -B $gbBuild @common
if ($LASTEXITCODE -ne 0) { throw "CMake configure failed for the mGBA core ($LASTEXITCODE)" }
& cmake --build $gbBuild --target pokedaisy_gbc_libretro -j $Jobs
if ($LASTEXITCODE -ne 0) { throw "mGBA core build failed ($LASTEXITCODE)" }

Write-Host "Configuring PokeDaisy DSperate core..."
& cmake -S (Join-Path $repo "linux/libretro-ds") -B $dsBuild @common `
  -DBUILD_TESTING=ON -DDSPERATE_GLES=OFF -DDSPERATE_VULKAN=OFF
if ($LASTEXITCODE -ne 0) { throw "CMake configure failed for the DS core ($LASTEXITCODE)" }
& cmake --build $dsBuild -j $Jobs
if ($LASTEXITCODE -ne 0) { throw "DS core and validation targets failed to build ($LASTEXITCODE)" }
& ctest --test-dir $dsBuild --output-on-failure
if ($LASTEXITCODE -ne 0) { throw "DS Windows validation suite failed ($LASTEXITCODE)" }

$artifacts = @(
  @{ Systems = @("GBC", "GBA"); Source = Join-Path $gbBuild "pokedaisy_gbc_libretro.dll"; Name = "pokedaisy_gbc_libretro.dll" },
  @{ Systems = @("DS"); Source = Join-Path $dsBuild "pokedaisy_ds_libretro.dll"; Name = "pokedaisy_ds_libretro.dll" }
)
foreach ($artifact in $artifacts) {
  if (-not (Test-Path -LiteralPath $artifact.Source)) { throw "Expected core DLL not found: $($artifact.Source)" }
  $destination = Join-Path $dist $artifact.Name
  Copy-Item -LiteralPath $artifact.Source -Destination $destination -Force
  $artifact["Probes"] = [ordered]@{}
  foreach ($system in $artifact.Systems) {
    $probeJson = & $PlayerExe --probe-core $destination $system 2>$null
    if ($LASTEXITCODE -ne 0) { throw "Core probe process failed for $system ($LASTEXITCODE)" }
    $probe = ($probeJson -join "`n") | ConvertFrom-Json
    if ($probe.status -ne "loadable" -or -not $probe.system_match) {
      throw "Core probe rejected ${system}: $($probe | ConvertTo-Json -Compress)"
    }
    $artifact.Probes[$system] = $probe
  }
  $artifact["Sha256"] = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-GitCommit([string]$Path) {
  $value = & git -C $Path rev-parse HEAD
  if ($LASTEXITCODE -ne 0) { throw "Could not read source commit: $Path" }
  return $value.Trim()
}
function Get-FileRecord([string]$RelativePath) {
  $path = Join-Path $repo $RelativePath
  return [ordered]@{
    path = $RelativePath
    sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
  }
}

$gcc = (& $mingwC --version | Select-Object -First 1).Trim()
$cmakeVersion = (& cmake --version | Select-Object -First 1).Trim()
$ninjaVersion = (& ninja --version | Select-Object -First 1).Trim()
$artifactRecords = foreach ($artifact in $artifacts) {
  $probeRecords = [ordered]@{}
  foreach ($system in $artifact.Systems) {
    $probe = $artifact.Probes[$system]
    $probeRecords[$system] = [ordered]@{
      status = $probe.status
      system_match = $probe.system_match
      architecture = $probe.architecture
      api_version = $probe.api_version
      library_name = $probe.library_name
    }
  }
  [ordered]@{
    systems = $artifact.Systems
    file = $artifact.Name
    sha256 = $artifact.Sha256
    probes = $probeRecords
  }
}
$manifest = [ordered]@{
  generated_at_utc = [DateTime]::UtcNow.ToString("o")
  repository_commit = Get-GitCommit $repo
  sources = [ordered]@{
    mgba = Get-GitCommit (Join-Path $repo "third_party/mgba")
    dsperate_wrapper = Get-GitCommit (Join-Path $repo "third_party/dsperate-lr")
    dsperate = Get-GitCommit (Join-Path $repo "third_party/dsperate-lr/DSperate")
  }
  toolchain = [ordered]@{ gcc = $gcc; cmake = $cmakeVersion; ninja = $ninjaVersion }
  licenses = @(
    (Get-FileRecord "third_party/mgba/LICENSE"),
    (Get-FileRecord "third_party/dsperate-lr/LICENSE"),
    (Get-FileRecord "third_party/dsperate-lr/DSperate/LICENSE")
  )
  options = [ordered]@{
    target = "Windows x86_64 GNU/MinGW"
    ds_jit = "disabled"
    ds_gles = "disabled"
    ds_vulkan = "disabled"
    binaries_are_local_build_outputs = $true
  }
  artifacts = @($artifactRecords)
  verification = [ordered]@{ ds_ctest = "passed"; probe = "loadable and expected system matched" }
}
$manifestPath = Join-Path $dist "provenance.json"
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $manifestPath -Encoding utf8
Write-Host "Built and probed both cores. Local artifacts and provenance: $dist"
