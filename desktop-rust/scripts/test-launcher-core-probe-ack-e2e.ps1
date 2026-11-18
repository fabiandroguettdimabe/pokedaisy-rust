param(
  [Parameter(Mandatory = $true)][string]$LauncherExe,
  [string]$Node = "node"
)

$ErrorActionPreference = "Stop"
$launcherExePath = (Resolve-Path -LiteralPath $LauncherExe).Path
$repo = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$probeScript = Join-Path $PSScriptRoot "test-core-manager-probes.mjs"
$tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd("\") + "\"
$dataRoot = [IO.Path]::GetFullPath((Join-Path $tempBase ("vyntra-core-probe-ack-" + [guid]::NewGuid().ToString("N"))))
if (-not $dataRoot.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase)) {
  throw "Temporary data root escaped the system temp directory: $dataRoot"
}
[IO.Directory]::CreateDirectory($dataRoot) | Out-Null

$userCoreRoot = Join-Path $env:USERPROFILE "Documents/Vyntra/cores"
$corePaths = @(
  (Join-Path $userCoreRoot "mgba_libretro.dll"),
  (Join-Path $userCoreRoot "mgba_libretro.dll"),
  (Join-Path $userCoreRoot "melondsds_libretro.dll")
)
foreach ($corePath in $corePaths) {
  if (-not (Test-Path -LiteralPath $corePath -PathType Leaf)) {
    throw "Required local core is missing: $corePath"
  }
}
$configDir = Join-Path $dataRoot "data"
[IO.Directory]::CreateDirectory($configDir) | Out-Null
@{ schema = 1; cores = $corePaths } |
  ConvertTo-Json -Depth 4 |
  Set-Content -LiteralPath (Join-Path $configDir "vyntra-settings.json") -Encoding utf8

$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = ([Net.IPEndPoint]$listener.LocalEndpoint).Port
$listener.Stop()
$cdpUrl = "http://127.0.0.1:$port"
$start = [Diagnostics.ProcessStartInfo]::new($launcherExePath)
$start.UseShellExecute = $false
$start.ArgumentList.Add("--data")
$start.ArgumentList.Add($dataRoot)
$start.Environment["WEBVIEW2_USER_DATA_FOLDER"] = Join-Path $dataRoot "webview-profile"
$start.Environment["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = "--remote-debugging-port=$port"
$launcherProcess = $null
$previousCdp = $env:VYNTRA_CORE_CHECK_CDP_URL
try {
  $launcherProcess = [Diagnostics.Process]::Start($start)
  $targets = $null
  for ($attempt = 0; $attempt -lt 120; $attempt++) {
    $launcherProcess.Refresh()
    if ($launcherProcess.HasExited) { throw "Isolated Launcher exited before WebView2 started" }
    try {
      $targets = Invoke-RestMethod -Uri "$cdpUrl/json/list" -TimeoutSec 1
      if ($targets | Where-Object { $_.type -eq "page" }) { break }
    } catch { }
    Start-Sleep -Milliseconds 250
  }
  if (-not ($targets | Where-Object { $_.type -eq "page" })) {
    throw "Isolated Launcher WebView2 CDP endpoint did not become ready"
  }
  $env:VYNTRA_CORE_CHECK_CDP_URL = $cdpUrl
  & $Node $probeScript
  if ($LASTEXITCODE -ne 0) { throw "Core probe terminal ACK smoke failed" }
}
finally {
  $env:VYNTRA_CORE_CHECK_CDP_URL = $previousCdp
  if ($launcherProcess) {
    $launcherProcess.Refresh()
    if (-not $launcherProcess.HasExited) {
      $launcherProcess.Kill()
      $launcherProcess.WaitForExit(5000) | Out-Null
    }
    $launcherProcess.Dispose()
  }
  if (Test-Path -LiteralPath $dataRoot -PathType Container) {
    $verifiedRoot = [IO.Path]::GetFullPath($dataRoot)
    if (-not $verifiedRoot.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase)) {
      throw "Refusing to remove data outside temp: $verifiedRoot"
    }
    Remove-Item -LiteralPath $verifiedRoot -Recurse -Force
  }
}
