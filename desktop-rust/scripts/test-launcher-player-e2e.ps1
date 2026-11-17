param(
  [Parameter(Mandatory = $true)][string]$PlayerExe,
  [Parameter(Mandatory = $true)][string]$CoreDirectory,
  [string]$DataRoot = (Join-Path ([IO.Path]::GetTempPath()) ("vyntra-launcher-player-" + [DateTime]::UtcNow.Ticks)),
  [string]$Python = "python",
  [string]$Node = "node"
)

$ErrorActionPreference = "Stop"
$playerExePath = (Resolve-Path -LiteralPath $PlayerExe).Path
$coreDirPath = (Resolve-Path -LiteralPath $CoreDirectory).Path
$repo = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$uiDirectory = Join-Path $repo "desktop-rust/ui"
$playwrightModule = Join-Path $uiDirectory "node_modules/playwright-core/index.mjs"
if (-not (Test-Path -LiteralPath $playwrightModule)) { throw "Install the Desktop UI dependencies first: $uiDirectory (npm ci)" }
$data = [IO.Path]::GetFullPath($DataRoot)
[IO.Directory]::CreateDirectory($data) | Out-Null
$rom = Join-Path $data "diagnostic-gba-input.gba"
& $Python (Join-Path $repo "linux/make_diagnostic_gba_input.py") $rom
if ($LASTEXITCODE -ne 0) { throw "Could not generate the GBA input diagnostic ROM" }

$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = ([Net.IPEndPoint]$listener.LocalEndpoint).Port
$listener.Stop()
$cdpUrl = "http://127.0.0.1:$port"
$clickScript = Join-Path $data "click-play.mjs"
$companionScript = Join-Path $data "companion-commands.mjs"
$moduleUrl = ([Uri]::new($playwrightModule)).AbsoluteUri
@"
import { chromium } from '$moduleUrl';
const browser = await chromium.connectOverCDP(process.env.VYNTRA_E2E_CDP_URL);
const pages = browser.contexts().flatMap(context => context.pages());
const launcher = pages[0];
if (!launcher) throw new Error('Launcher WebView2 page is missing');
await launcher.getByRole('navigation', { name: 'Navegación principal' }).getByRole('button', { name: 'Biblioteca' }).click();
await launcher.getByRole('heading', { name: 'PD INPUT GBA' }).waitFor({ timeout: 10000 });
await launcher.getByRole('button', { name: 'Jugar', exact: true }).click();
console.log('launcher-play-clicked');
process.exit(0);
"@ | Set-Content -LiteralPath $clickScript -Encoding utf8
@"
import { chromium } from '$moduleUrl';
const browser = await chromium.connectOverCDP(process.env.VYNTRA_E2E_CDP_URL);
const launcher = browser.contexts().flatMap(context => context.pages())[0];
if (!launcher) throw new Error('Launcher WebView2 page is missing');
await launcher.getByRole('navigation', { name: 'Navegación principal' }).getByRole('button', { name: 'Biblioteca' }).click();
await launcher.getByRole('button', { name: 'Jugar', exact: true }).click();
await launcher.getByRole('alert').filter({ hasText: 'Ya hay una partida abierta' }).waitFor({ timeout: 5000 });
console.log('second-play-rejected-while-player-active');
await launcher.getByRole('navigation', { name: 'Navegación principal' }).getByRole('button', { name: 'Acompañante' }).click();
await launcher.getByRole('heading', { name: 'Vyntra Companion' }).waitFor({ timeout: 10000 });
const session = launcher.locator('.companion-session');
await session.waitFor({ timeout: 15000 });
await session.getByText(/Core ·/).waitFor({ timeout: 15000 });
await session.getByRole('button', { name: 'Pausar' }).click();
await session.getByText(/En pausa/).waitFor({ timeout: 5000 });
await session.getByRole('button', { name: 'Reanudar' }).click();
await session.getByText('En pausa').waitFor({ state: 'hidden', timeout: 5000 });
await session.getByRole('button', { name: 'Guardar estado' }).click();
await launcher.getByText('Estado guardado en Player.').waitFor({ timeout: 10000 });
await session.getByRole('button', { name: 'Cargar estado' }).click();
await launcher.getByText('Estado cargado en Player.').waitFor({ timeout: 10000 });
await session.getByRole('button', { name: 'Detener partida' }).click();
console.log('companion-pause-save-load-stop-passed');
process.exit(0);
"@ | Set-Content -LiteralPath $companionScript -Encoding utf8

if (-not ("VyntraLauncherPlayerSmoke" -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class VyntraLauncherPlayerSmoke {
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd,uint message,IntPtr wParam,IntPtr lParam);
}
'@
}

function Invoke-LauncherPlay {
  $env:VYNTRA_E2E_CDP_URL = $cdpUrl
  & $Node $clickScript
  if ($LASTEXITCODE -ne 0) { throw "Playwright could not click Play in Launcher" }
}
function Invoke-CompanionCommands {
  $env:VYNTRA_E2E_CDP_URL = $cdpUrl
  & $Node $companionScript
  if ($LASTEXITCODE -ne 0) { throw "Companion commands did not complete successfully" }
}
function Wait-PlayerChild([int]$ParentId) {
  for ($attempt = 0; $attempt -lt 120; $attempt++) {
    $child = Get-CimInstance Win32_Process -Filter "ParentProcessId = $ParentId" |
      Where-Object { $_.Name -eq "vyntra-desktop.exe" -and $_.CommandLine -like "*--emulator*" } |
      Select-Object -First 1
    if ($child) {
      $process = Get-Process -Id $child.ProcessId -ErrorAction SilentlyContinue
      if ($process) {
        $process.Refresh()
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) { return $process }
      }
    }
    $launcherProcess = Get-Process -Id $ParentId -ErrorAction SilentlyContinue
    if (-not $launcherProcess) { throw "Launcher exited before starting Player" }
    Start-Sleep -Milliseconds 250
  }
  throw "Launcher did not start a visible Player process within 30 seconds"
}
function Close-Player([Diagnostics.Process]$Process) {
  [void][VyntraLauncherPlayerSmoke]::SendMessage($Process.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
  if (-not $Process.WaitForExit(15000)) { throw "Player did not close after WM_CLOSE" }
}

$start = [Diagnostics.ProcessStartInfo]::new($playerExePath)
$start.UseShellExecute = $false
$start.ArgumentList.Add("--data")
$start.ArgumentList.Add($data)
$start.ArgumentList.Add($rom)
$start.Environment["VYNTRA_CORE_DIR"] = $coreDirPath
# The debug port is scoped to this process and bound to loopback for this smoke only.
$start.Environment["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = "--remote-debugging-port=$port"
$launcherProcess = [Diagnostics.Process]::Start($start)
$playerProcess = $null
try {
  $targets = $null
  for ($attempt = 0; $attempt -lt 120; $attempt++) {
    $launcherProcess.Refresh()
    if ($launcherProcess.HasExited) { throw "Launcher exited before WebView2 started" }
    try {
      $targets = Invoke-RestMethod -Uri "$cdpUrl/json/list" -TimeoutSec 1
      if ($targets | Where-Object { $_.type -eq "page" }) { break }
    } catch { }
    Start-Sleep -Milliseconds 250
  }
  if (-not ($targets | Where-Object { $_.type -eq "page" })) { throw "WebView2 CDP endpoint did not become ready" }

  Invoke-LauncherPlay
  $playerProcess = Wait-PlayerChild $launcherProcess.Id
  $playerCommand = (Get-CimInstance Win32_Process -Filter "ProcessId = $($playerProcess.Id)").CommandLine
  if ($playerCommand -notmatch "--pipe\s+\S+" -or $playerCommand -notmatch "--session-id\s+sess-" -or
      $playerCommand -notmatch "--session-epoch\s+1") {
    throw "Player was not launched with the authenticated session transport arguments"
  }

  [void][VyntraLauncherPlayerSmoke]::SendMessage($playerProcess.MainWindowHandle, 0x0100, [IntPtr]0x5A, [IntPtr]0x002C0001)
  Start-Sleep -Milliseconds 400
  [void][VyntraLauncherPlayerSmoke]::SendMessage($playerProcess.MainWindowHandle, 0x0101, [IntPtr]0x5A, [IntPtr]0xC02C0001)
  Start-Sleep -Milliseconds 400
  Invoke-CompanionCommands
  $activePlayers = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($launcherProcess.Id)" |
    Where-Object { $_.Name -eq "vyntra-desktop.exe" -and $_.CommandLine -like "*--emulator*" })
  if ($activePlayers.Count -ne 1 -or $activePlayers[0].ProcessId -ne $playerProcess.Id) {
    throw "A second Play created another Player or replaced the active process"
  }
  if (-not $playerProcess.WaitForExit(15000)) { throw "Companion Stop did not close Player" }
  $playerProcess.Dispose()
  $playerProcess = $null
  Start-Sleep -Milliseconds 800

  $romHash = (Get-FileHash -LiteralPath $rom -Algorithm SHA256).Hash.ToLowerInvariant()
  $corePath = Join-Path $coreDirPath "pokedaisy_gbc_libretro.dll"
  $coreHash = (Get-FileHash -LiteralPath $corePath -Algorithm SHA256).Hash.ToLowerInvariant()
  $saveDir = Join-Path $data ("data/saves/by-id/" + $romHash.Substring(0, 16))
  $batteryPath = Join-Path $saveDir "battery.sav"
  $manifestPath = Join-Path $saveDir "manifest.json"
  if (-not (Test-Path -LiteralPath $batteryPath) -or -not (Test-Path -LiteralPath $manifestPath)) {
    throw "Launcher session ended without committing the Player SRAM and manifest"
  }
  $bytes = [IO.File]::ReadAllBytes($batteryPath)
  $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
  $saveHash = (Get-FileHash -LiteralPath $batteryPath -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($bytes.Length -ne 32768 -or $bytes[0] -ne 0xA5 -or $manifest.status -ne "committed" -or
      $manifest.sha256 -ne $saveHash -or $manifest.rom_sha256 -ne $romHash -or $manifest.core_hash -ne $coreHash) {
    throw "The Launcher session save does not match the expected input marker and ROM/core hashes"
  }
  $stateDir = Join-Path $data ("data/states/by-id/" + $romHash.Substring(0, 16))
  $statePath = Join-Path $stateDir "slot1.ss"
  $stateManifestPath = Join-Path $stateDir "state.manifest.json"
  if (-not (Test-Path -LiteralPath $statePath) -or -not (Test-Path -LiteralPath $stateManifestPath)) {
    throw "Companion Save State did not commit a state and manifest"
  }
  $stateBytes = [IO.File]::ReadAllBytes($statePath)
  $stateManifest = Get-Content -LiteralPath $stateManifestPath -Raw | ConvertFrom-Json
  $stateHash = (Get-FileHash -LiteralPath $statePath -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($stateBytes.Length -eq 0 -or $stateManifest.status -ne "committed" -or
      $stateManifest.sha256 -ne $stateHash -or $stateManifest.rom_sha256 -ne $romHash -or
      $stateManifest.core_hash -ne $coreHash) {
    throw "Companion state manifest does not match the committed state payload and core"
  }
  if ($launcherProcess.HasExited) { throw "Launcher exited when Player closed instead of returning to its window" }

  # A second Play proves the first pipe/session and save locks were released cleanly.
  Invoke-LauncherPlay
  $playerProcess = Wait-PlayerChild $launcherProcess.Id
  Close-Player $playerProcess
  $playerProcess.Dispose()
  $playerProcess = $null

  [PSCustomObject]@{
    result = "PASS"
    flow = "Launcher UI → reject concurrent Play → Companion pause/save/load/stop → committed SRAM+state → Play after Stop"
    session_arguments = "pipe, nonce, session id, epoch"
    input = "Z → libretro A"
    sram_marker = "0xA5"
    bytes = $bytes.Length
    manifest = $manifest.status
    state_bytes = $stateBytes.Length
    state_manifest = $stateManifest.status
    rom_sha256 = $romHash
    core_sha256 = $coreHash
    save_sha256 = $saveHash
    launcher_pid = $launcherProcess.Id
    data_root = $data
  }
}
finally {
  if ($playerProcess) {
    if (-not $playerProcess.HasExited) {
      [void][VyntraLauncherPlayerSmoke]::SendMessage($playerProcess.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
      if (-not $playerProcess.WaitForExit(5000)) { $playerProcess.Kill() }
    }
    $playerProcess.Dispose()
  }
  if ($launcherProcess -and -not $launcherProcess.HasExited) {
    [void]$launcherProcess.CloseMainWindow()
    if (-not $launcherProcess.WaitForExit(5000)) { $launcherProcess.Kill() }
    $launcherProcess.Dispose()
  }
  Remove-Item Env:VYNTRA_E2E_CDP_URL -ErrorAction SilentlyContinue
}
