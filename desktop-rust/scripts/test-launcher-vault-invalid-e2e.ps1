param(
  [Parameter(Mandatory = $true)][string]$LauncherExe,
  [string]$DataRoot,
  [string]$Node = "node"
)

$ErrorActionPreference = "Stop"
$launcherExePath = (Resolve-Path -LiteralPath $LauncherExe).Path
$repo = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$uiDirectory = Join-Path $repo "desktop-rust/ui"
$playwrightModule = Join-Path $uiDirectory "node_modules/playwright-core/index.mjs"
if (-not (Test-Path -LiteralPath $playwrightModule)) { throw "Install the Desktop UI dependencies first: $uiDirectory (npm ci)" }

$ownsDataRoot = [string]::IsNullOrWhiteSpace($DataRoot)
if ($ownsDataRoot) { $DataRoot = Join-Path ([IO.Path]::GetTempPath()) ("vyntra-vault-invalid-" + [DateTime]::UtcNow.Ticks) }
$data = [IO.Path]::GetFullPath($DataRoot)
[IO.Directory]::CreateDirectory($data) | Out-Null
$invalidBundle = Join-Path $data "invalid-vault-bundle.zip"
[IO.File]::WriteAllBytes($invalidBundle, [Text.Encoding]::UTF8.GetBytes("not a ZIP bundle"))

$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = ([Net.IPEndPoint]$listener.LocalEndpoint).Port
$listener.Stop()
$cdpUrl = "http://127.0.0.1:$port"
$uiScript = Join-Path $data "open-vault-import.mjs"
$moduleUrl = ([Uri]::new($playwrightModule)).AbsoluteUri
@"
import { chromium } from '$moduleUrl';
const browser = await chromium.connectOverCDP(process.env.VYNTRA_E2E_CDP_URL);
const launcher = browser.contexts().flatMap(context => context.pages())[0];
if (!launcher) throw new Error('Launcher WebView2 page is missing');
await launcher.getByRole('navigation', { name: 'Navegación principal' }).getByRole('button', { name: 'Vyntra Vault' }).click();
await launcher.getByRole('heading', { name: 'Vyntra Vault' }).waitFor({ timeout: 10000 });
await launcher.getByRole('button', { name: 'Importar paquete' }).click();
console.log('vault-import-dialog-requested');
process.exit(0);
"@ | Set-Content -LiteralPath $uiScript -Encoding utf8

if (-not ("VyntraVaultInvalidDialogSmoke" -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class VyntraVaultInvalidDialogSmoke {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumWindowsProc callback, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hWnd, StringBuilder text, int count);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hWnd, int itemId);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
  [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")]
  public static extern IntPtr SendText(IntPtr hWnd, uint message, IntPtr wParam, [MarshalAs(UnmanagedType.LPWStr)] string text);
  [DllImport("user32.dll", EntryPoint="SendMessageW")]
  public static extern IntPtr SendClick(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);
  public static IntPtr[] WindowsForProcess(int processId) {
    var found = new List<IntPtr>();
    EnumWindows((hWnd, lParam) => {
      uint owner;
      GetWindowThreadProcessId(hWnd, out owner);
      if (owner == processId && IsWindowVisible(hWnd)) found.Add(hWnd);
      return true;
    }, IntPtr.Zero);
    return found.ToArray();
  }
  public static IntPtr[] Children(IntPtr parent) {
    var found = new List<IntPtr>();
    EnumChildWindows(parent, (hWnd, lParam) => { found.Add(hWnd); return true; }, IntPtr.Zero);
    return found.ToArray();
  }
  public static string Title(IntPtr hWnd) {
    var text = new StringBuilder(512);
    GetWindowText(hWnd, text, text.Capacity);
    return text.ToString();
  }
  public static string ClassName(IntPtr hWnd) {
    var text = new StringBuilder(256);
    GetClassName(hWnd, text, text.Capacity);
    return text.ToString();
  }
}
'@
}

$start = [Diagnostics.ProcessStartInfo]::new($launcherExePath)
$start.UseShellExecute = $false
$start.ArgumentList.Add("--data")
$start.ArgumentList.Add($data)
$start.Environment["WEBVIEW2_USER_DATA_FOLDER"] = Join-Path $data "webview-profile"
$start.Environment["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = "--remote-debugging-port=$port"
$launcherProcess = [Diagnostics.Process]::Start($start)
try {
  $targets = $null
  for ($attempt = 0; $attempt -lt 120; $attempt++) {
    $launcherProcess.Refresh()
    if ($launcherProcess.HasExited) { throw "Smoke Launcher exited before WebView2 started" }
    try {
      $targets = Invoke-RestMethod -Uri "$cdpUrl/json/list" -TimeoutSec 1
      if ($targets | Where-Object { $_.type -eq "page" }) { break }
    } catch { }
    Start-Sleep -Milliseconds 250
  }
  if (-not ($targets | Where-Object { $_.type -eq "page" })) { throw "Smoke Launcher WebView2 CDP endpoint did not become ready" }

  $env:VYNTRA_E2E_CDP_URL = $cdpUrl
  & $Node $uiScript
  if ($LASTEXITCODE -ne 0) { throw "Playwright could not request Vault import" }

  $dialog = $null
  for ($attempt = 0; $attempt -lt 120; $attempt++) {
    $dialog = [VyntraVaultInvalidDialogSmoke]::WindowsForProcess($launcherProcess.Id) |
      Where-Object { [VyntraVaultInvalidDialogSmoke]::Title($_) -like "*Vyntra Vault*" } |
      Select-Object -First 1
    if ($dialog) { break }
    Start-Sleep -Milliseconds 250
  }
  if (-not $dialog) {
    $titles = [VyntraVaultInvalidDialogSmoke]::WindowsForProcess($launcherProcess.Id) |
      ForEach-Object { [VyntraVaultInvalidDialogSmoke]::Title($_) }
    throw "Vault native file picker did not appear for smoke Launcher PID $($launcherProcess.Id). Visible windows: $($titles -join ' | ')"
  }

  $nativeChildren = [VyntraVaultInvalidDialogSmoke]::Children($dialog)
  $nativeDetails = $nativeChildren |
    ForEach-Object { "id=$([VyntraVaultInvalidDialogSmoke]::GetDlgCtrlID($_)) class=$([VyntraVaultInvalidDialogSmoke]::ClassName($_)) title='$([VyntraVaultInvalidDialogSmoke]::Title($_))'" }
  $filenameField = $nativeChildren |
    Where-Object { [VyntraVaultInvalidDialogSmoke]::GetDlgCtrlID($_) -eq 1148 -and [VyntraVaultInvalidDialogSmoke]::ClassName($_) -eq "Edit" } |
    Select-Object -First 1
  if (-not $filenameField) { throw "Could not identify the native filename Edit control. Win32 controls: $($nativeDetails -join '; ')" }
  [void][VyntraVaultInvalidDialogSmoke]::SendText($filenameField, 0x000C, [IntPtr]::Zero, $invalidBundle)
  $openButton = [VyntraVaultInvalidDialogSmoke]::GetDlgItem($dialog, 1)
  if (-not $openButton) { throw "Could not identify the native Open button. Win32 controls: $($nativeDetails -join '; ')" }
  [void][VyntraVaultInvalidDialogSmoke]::SendClick($openButton, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)

  $alertScript = Join-Path $data "assert-vault-error.mjs"
  @"
import { chromium } from '$moduleUrl';
const browser = await chromium.connectOverCDP(process.env.VYNTRA_E2E_CDP_URL);
const launcher = browser.contexts().flatMap(context => context.pages())[0];
if (!launcher) throw new Error('Launcher WebView2 page disappeared');
const alert = launcher.getByRole('alert');
await alert.waitFor({ timeout: 15000 });
const message = (await alert.innerText()).trim();
if (!message || !/zip|archivo|bundle|archivo/i.test(message)) throw new Error('Invalid Vault archive error was not surfaced in the accessible alert: ' + message);
await alert.getByRole('button', { name: 'Cerrar aviso' }).waitFor({ timeout: 5000 });
console.log('vault-invalid-archive-alert=' + message.replace(/\s+/g, ' '));
process.exit(0);
"@ | Set-Content -LiteralPath $alertScript -Encoding utf8
  & $Node $alertScript
  if ($LASTEXITCODE -ne 0) { throw "Launcher did not surface the invalid Vault archive error" }

  [PSCustomObject]@{
    result = "PASS"
    flow = "Launcher UI → native Vault file picker → invalid ZIP → accessible terminal error"
    launcher_pid = $launcherProcess.Id
    invalid_bundle = $invalidBundle
  }
}
finally {
  Remove-Item Env:VYNTRA_E2E_CDP_URL -ErrorAction SilentlyContinue
  if ($launcherProcess -and -not $launcherProcess.HasExited) {
    [void]$launcherProcess.CloseMainWindow()
    if (-not $launcherProcess.WaitForExit(5000)) { $launcherProcess.Kill() }
    $launcherProcess.Dispose()
  }
  if ($ownsDataRoot -and (Test-Path -LiteralPath $data)) {
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    $dataName = [IO.Path]::GetFileName($data)
    if ($data.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and $dataName.StartsWith('vyntra-vault-invalid-', [StringComparison]::Ordinal)) {
      Remove-Item -LiteralPath $data -Recurse -Force
    } else {
      throw "Refusing to remove smoke data outside its generated temp folder: $data"
    }
  }
}
