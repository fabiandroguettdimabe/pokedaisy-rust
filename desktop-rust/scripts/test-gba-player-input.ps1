param(
  [Parameter(Mandatory = $true)][string]$PlayerExe,
  [Parameter(Mandatory = $true)][string]$CoreDirectory,
  [string]$DataRoot = (Join-Path ([IO.Path]::GetTempPath()) ("vyntra-gba-input-" + [DateTime]::UtcNow.Ticks)),
  [string]$Python = "python"
)

$ErrorActionPreference = "Stop"
$player = (Resolve-Path -LiteralPath $PlayerExe).Path
$cores = (Resolve-Path -LiteralPath $CoreDirectory).Path
$repo = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$data = [IO.Path]::GetFullPath($DataRoot)
[IO.Directory]::CreateDirectory($data) | Out-Null
$rom = Join-Path $data "diagnostic-gba-input.gba"
$generator = Join-Path $repo "linux/make_diagnostic_gba_input.py"
& $Python $generator $rom
if ($LASTEXITCODE -ne 0) { throw "Could not generate the GBA input diagnostic ROM" }

if (-not ("VyntraInputSmoke" -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class VyntraInputSmoke {
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);
}
'@
}

$start = [Diagnostics.ProcessStartInfo]::new($player)
$start.UseShellExecute = $false
$start.ArgumentList.Add("--emulator")
$start.ArgumentList.Add("--data")
$start.ArgumentList.Add($data)
$start.ArgumentList.Add($rom)
$start.Environment["VYNTRA_CORE_DIR"] = $cores
$process = [Diagnostics.Process]::Start($start)
try {
  $window = [IntPtr]::Zero
  for ($attempt = 0; $attempt -lt 120; $attempt++) {
    $process.Refresh()
    if ($process.HasExited) { throw "Player exited before creating its SDL window" }
    $window = $process.MainWindowHandle
    if ($window -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 250
  }
  if ($window -eq [IntPtr]::Zero) { throw "Player did not create an SDL window within 30 seconds" }

  # Z is the default keyboard binding for libretro A. Dispatch a normal Win32 key
  # message to the real SDL window and let Player pump it through the core.
  [void][VyntraInputSmoke]::SendMessage($window, 0x0100, [IntPtr]0x5A, [IntPtr]0x002C0001)
  Start-Sleep -Milliseconds 350
  [void][VyntraInputSmoke]::SendMessage($window, 0x0101, [IntPtr]0x5A, [IntPtr]0xC02C0001)
  Start-Sleep -Milliseconds 500
  [void][VyntraInputSmoke]::SendMessage($window, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
  if (-not $process.WaitForExit(15000)) { throw "Player did not close after WM_CLOSE" }

  $romHash = (Get-FileHash -LiteralPath $rom -Algorithm SHA256).Hash.ToLowerInvariant()
  $corePath = Join-Path $cores "pokedaisy_gbc_libretro.dll"
  if (-not (Test-Path -LiteralPath $corePath)) { throw "Shared mGBA core was not found: $corePath" }
  $coreHash = (Get-FileHash -LiteralPath $corePath -Algorithm SHA256).Hash.ToLowerInvariant()
  $saveDirectory = Join-Path $data ("data/saves/by-id/" + $romHash.Substring(0, 16))
  $battery = Join-Path $saveDirectory "battery.sav"
  $manifestPath = Join-Path $saveDirectory "manifest.json"
  if (-not (Test-Path -LiteralPath $battery)) { throw "Player did not commit the diagnostic battery save" }
  $bytes = [IO.File]::ReadAllBytes($battery)
  $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
  $batteryHash = (Get-FileHash -LiteralPath $battery -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($bytes.Length -ne 32768 -or $bytes[0] -ne 0xA5) { throw "Z→A did not write marker 0xA5 to the committed SRAM" }
  if ($manifest.status -ne "committed" -or $manifest.sha256 -ne $batteryHash -or
      $manifest.rom_sha256 -ne $romHash -or $manifest.core_hash -ne $coreHash) {
    throw "The save manifest does not match the ROM, core, and committed battery payload"
  }
  [PSCustomObject]@{
    result = "PASS"
    input = "Z → libretro A"
    sram_marker = "0xA5"
    bytes = $bytes.Length
    manifest = $manifest.status
    rom_sha256 = $romHash
    core_sha256 = $coreHash
    save_sha256 = $batteryHash
    data_root = $data
  }
}
finally {
  if (-not $process.HasExited) {
    [void][VyntraInputSmoke]::SendMessage($process.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
    if (-not $process.WaitForExit(5000)) { $process.Kill() }
  }
  $process.Dispose()
}
