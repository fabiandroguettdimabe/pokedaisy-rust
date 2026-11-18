param(
  [Parameter(Mandatory = $true)][string]$LauncherExe,
  [Parameter(Mandatory = $true)][string]$RomPath,
  [Parameter(Mandatory = $true)][string]$LibraryPath,
  [string]$Node = "node"
)

$ErrorActionPreference = "Stop"
$launcherExePath = (Resolve-Path -LiteralPath $LauncherExe).Path
$romFile = (Resolve-Path -LiteralPath $RomPath).Path
$libraryDirectory = (Resolve-Path -LiteralPath $LibraryPath).Path
$playwrightModule = Join-Path (Resolve-Path (Join-Path $PSScriptRoot "../ui")).Path "node_modules/playwright-core/index.mjs"
if (-not (Test-Path -LiteralPath $playwrightModule)) {
  throw "Install Desktop UI dependencies first"
}
if (-not (Test-Path -LiteralPath $romFile -PathType Leaf)) {
  throw "The real ROM file is missing: $romFile"
}

$tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd("\") + "\"
$dataRoot = [IO.Path]::GetFullPath((Join-Path $tempBase ("vyntra-real-rom-" + [guid]::NewGuid().ToString("N"))))
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
@{
  schema = 1
  library = $libraryDirectory
  saves = Join-Path $dataRoot "saves"
  states = Join-Path $dataRoot "states"
  system = Join-Path $dataRoot "system"
  recursive = $false
  cores = $corePaths
} | ConvertTo-Json -Depth 4 |
  Set-Content -LiteralPath (Join-Path $configDir "vyntra-settings.json") -Encoding utf8

$moduleUrl = ([Uri]::new($playwrightModule)).AbsoluteUri
$uiScript = Join-Path $dataRoot "play-real-rom.mjs"
@"
import assert from 'node:assert/strict';
import { chromium } from '$moduleUrl';
const browser = await chromium.connectOverCDP(process.env.VYNTRA_REAL_ROM_CDP_URL);
try {
  const page = browser.contexts().flatMap(context => context.pages())[0];
  assert.ok(page, 'Launcher WebView2 page is missing');
  await page.getByRole('heading', { name: 'CASCADE7', exact: true }).waitFor({ timeout: 15000 });
  await page.waitForFunction(() => typeof window.vyntraPlayerSnapshot === 'function');
  await page.evaluate(() => {
    const originalSnapshot = window.vyntraPlayerSnapshot;
    window.vyntraPlayerSnapshots = [];
    window.vyntraPlayerSnapshot = snapshot => {
      window.vyntraPlayerSnapshots.push(snapshot);
      originalSnapshot(snapshot);
    };
  });
  await page.locator('.game-card').filter({ hasText: 'CASCADE7' }).click();
  await page.getByRole('button', { name: 'Jugar', exact: true }).click();
  await page.waitForFunction(
    () => window.vyntraPlayerSnapshots.some(snapshot =>
      snapshot.title === 'CASCADE7' &&
      snapshot.screen === 'playing' &&
      snapshot.core?.name === 'mGBA'
    ),
    undefined,
    { timeout: 20000 }
  );
  const snapshot = await page.evaluate(() => window.vyntraPlayerSnapshots.find(item =>
    item.title === 'CASCADE7' && item.screen === 'playing' && item.core?.name === 'mGBA'
  ));
  assert.equal(snapshot.core.version, '0.11-212-7a12d6d');
  assert.equal(snapshot.system, 'GBA');
  console.log(JSON.stringify({
    result: 'PASS',
    game: snapshot.title,
    system: snapshot.system,
    core: snapshot.core,
    screen: snapshot.screen,
  }));
} finally {
  await browser.close();
}
"@ | Set-Content -LiteralPath $uiScript -Encoding utf8

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
$previousCdp = $env:VYNTRA_REAL_ROM_CDP_URL
$previousGameTitle = $env:VYNTRA_REAL_ROM_TITLE
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
  $env:VYNTRA_REAL_ROM_CDP_URL = $cdpUrl
  $env:VYNTRA_REAL_ROM_TITLE = [IO.Path]::GetFileNameWithoutExtension($romFile)
  & $Node $uiScript
  if ($LASTEXITCODE -ne 0) { throw "Real ROM Play smoke failed" }
}
finally {
  $env:VYNTRA_REAL_ROM_CDP_URL = $previousCdp
  $env:VYNTRA_REAL_ROM_TITLE = $previousGameTitle
  $childProcesses = Get-CimInstance Win32_Process | Where-Object {
    $_.ExecutablePath -eq $launcherExePath -and
    $_.CommandLine.Contains($dataRoot) -and
    $_.CommandLine.Contains("--emulator")
  }
  foreach ($child in $childProcesses) {
    Stop-Process -Id $child.ProcessId -Force -ErrorAction SilentlyContinue
  }
  if ($launcherProcess) {
    $launcherProcess.Refresh()
    if (-not $launcherProcess.HasExited) {
      [void]$launcherProcess.CloseMainWindow()
      if (-not $launcherProcess.WaitForExit(5000)) {
        $launcherProcess.Kill()
        $launcherProcess.WaitForExit(5000) | Out-Null
      }
    }
    $launcherProcess.Dispose()
  }
  if (Test-Path -LiteralPath $dataRoot -PathType Container) {
    $verifiedRoot = [IO.Path]::GetFullPath($dataRoot)
    if (-not $verifiedRoot.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase)) {
      throw "Refusing to remove data outside temp: $verifiedRoot"
    }
    $webviewProcesses = @(Get-CimInstance Win32_Process)
    $ownedWebViewIds = [Collections.Generic.HashSet[int]]::new()
    foreach ($process in $webviewProcesses) {
      if ($process.Name -eq "msedgewebview2.exe" -and
          $process.CommandLine -and
          $process.CommandLine.Contains((Join-Path $dataRoot "webview-profile"))) {
        [void]$ownedWebViewIds.Add([int]$process.ProcessId)
      }
    }
    $changed = $true
    while ($changed) {
      $changed = $false
      foreach ($process in $webviewProcesses) {
        if ($process.Name -ne "msedgewebview2.exe") { continue }
        $processId = [int]$process.ProcessId
        $parentId = [int]$process.ParentProcessId
        if ($ownedWebViewIds.Contains($parentId) -and $ownedWebViewIds.Add($processId)) {
          $changed = $true
        }
        if ($ownedWebViewIds.Contains($processId) -and $ownedWebViewIds.Add($parentId)) {
          $parentProcess = $webviewProcesses | Where-Object { [int]$_.ProcessId -eq $parentId }
          if ($parentProcess.Name -eq "msedgewebview2.exe") { $changed = $true }
          else { [void]$ownedWebViewIds.Remove($parentId) }
        }
      }
    }
    $webviewProcessIds = @($ownedWebViewIds)
    foreach ($processId in $webviewProcessIds) {
      Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
    }
    foreach ($processId in $webviewProcessIds) {
      Wait-Process -Id $processId -Timeout 10 -ErrorAction SilentlyContinue
    }
    Start-Sleep -Milliseconds 500
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
      try {
        Remove-Item -LiteralPath $verifiedRoot -Recurse -Force -ErrorAction Stop
        break
      } catch {
        if ($attempt -eq 19) { throw }
        Start-Sleep -Milliseconds 250
      }
    }
  }
}
