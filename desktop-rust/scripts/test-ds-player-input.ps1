param(
  [Parameter(Mandatory = $true)][string]$PlayerExe,
  [Parameter(Mandatory = $true)][string]$CoreDirectory,
  [string]$DataRoot = (Join-Path ([IO.Path]::GetTempPath()) ("vyntra-ds-input-" + [DateTime]::UtcNow.Ticks)),
  [string]$Python = "python"
)

$ErrorActionPreference = "Stop"
$player = (Resolve-Path -LiteralPath $PlayerExe).Path
$cores = (Resolve-Path -LiteralPath $CoreDirectory).Path
$repo = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$data = [IO.Path]::GetFullPath($DataRoot)
[IO.Directory]::CreateDirectory($data) | Out-Null
$rom = Join-Path $data "diagnostic.nds"
& $Python (Join-Path $repo "linux/dsperate/make_diagnostic.py") $rom
if ($LASTEXITCODE -ne 0) { throw "Could not generate the DS input diagnostic ROM" }

if (-not ("VyntraDsInputSmoke" -as [type])) {
  Add-Type -TypeDefinition @'
using System;
public static class VyntraDsInputSmoke {
  [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);
  [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr hWnd);
  [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr hWnd, IntPtr hdc);
  [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)] public struct BITMAPINFOHEADER {
    public uint Size; public int Width,Height; public ushort Planes,BitCount; public uint Compression,SizeImage; public int XPelsPerMeter,YPelsPerMeter; public uint ColorsUsed,ColorsImportant;
  }
  [System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)] public struct BITMAPINFO { public BITMAPINFOHEADER Header; public uint Colors; }
  [System.Runtime.InteropServices.DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleDC(IntPtr hdc);
  [System.Runtime.InteropServices.DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleBitmap(IntPtr hdc,int width,int height);
  [System.Runtime.InteropServices.DllImport("gdi32.dll")] public static extern IntPtr SelectObject(IntPtr hdc,IntPtr obj);
  [System.Runtime.InteropServices.DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr obj);
  [System.Runtime.InteropServices.DllImport("gdi32.dll")] public static extern bool DeleteDC(IntPtr hdc);
  [System.Runtime.InteropServices.DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr dst,int x,int y,int width,int height,IntPtr src,int sx,int sy,uint operation);
  [System.Runtime.InteropServices.DllImport("gdi32.dll")] public static extern int GetDIBits(IntPtr hdc,IntPtr bitmap,uint start,uint lines,byte[] bits,ref BITMAPINFO info,uint usage);
  public static byte[] Capture(IntPtr hWnd) {
    RECT rect; if (!GetWindowRect(hWnd, out rect)) throw new InvalidOperationException("GetWindowRect failed");
    int width=rect.Right-rect.Left, height=rect.Bottom-rect.Top;
    if(width<=0||height<=0)throw new InvalidOperationException("Player window has no area");
    IntPtr screen=GetDC(IntPtr.Zero); if(screen==IntPtr.Zero)throw new InvalidOperationException("GetDC failed");
    IntPtr memory=IntPtr.Zero,bitmap=IntPtr.Zero,previous=IntPtr.Zero;
    try {
      memory=CreateCompatibleDC(screen);bitmap=CreateCompatibleBitmap(screen,width,height);
      if(memory==IntPtr.Zero||bitmap==IntPtr.Zero)throw new InvalidOperationException("Could not allocate a window capture bitmap");
      previous=SelectObject(memory,bitmap);
      if(!BitBlt(memory,0,0,width,height,screen,rect.Left,rect.Top,0x00CC0020))throw new InvalidOperationException("BitBlt failed");
      BITMAPINFO info=new BITMAPINFO();info.Header.Size=(uint)System.Runtime.InteropServices.Marshal.SizeOf(typeof(BITMAPINFOHEADER));
      info.Header.Width=width;info.Header.Height=-height;info.Header.Planes=1;info.Header.BitCount=32;info.Header.Compression=0;info.Header.SizeImage=(uint)(width*height*4);
      byte[] pixels=new byte[width*height*4];
      if(GetDIBits(memory,bitmap,0,(uint)height,pixels,ref info,0)!=height)throw new InvalidOperationException("GetDIBits failed");
      return pixels;
    } finally {
      if(previous!=IntPtr.Zero)SelectObject(memory,previous);if(bitmap!=IntPtr.Zero)DeleteObject(bitmap);if(memory!=IntPtr.Zero)DeleteDC(memory);ReleaseDC(IntPtr.Zero,screen);
    }
  }
  public static int ChangedPixels(byte[] a, byte[] b) {
    if(a.Length!=b.Length)throw new ArgumentException("Screenshot dimensions changed");
    int changed=0;
    for(int i=0;i<a.Length;i+=16)if(a[i]!=b[i]||a[i+1]!=b[i+1]||a[i+2]!=b[i+2])changed++;
    return changed;
  }
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

  Start-Sleep -Milliseconds 900
  $releasedBefore = [VyntraDsInputSmoke]::Capture($window)
  [void][VyntraDsInputSmoke]::SendMessage($window, 0x0100, [IntPtr]0x5A, [IntPtr]0x002C0001)
  Start-Sleep -Milliseconds 900
  $pressed = [VyntraDsInputSmoke]::Capture($window)
  [void][VyntraDsInputSmoke]::SendMessage($window, 0x0101, [IntPtr]0x5A, [IntPtr]0xC02C0001)
  Start-Sleep -Milliseconds 900
  $releasedAfter = [VyntraDsInputSmoke]::Capture($window)
  $pressChanges = [VyntraDsInputSmoke]::ChangedPixels($releasedBefore, $pressed)
  $releaseChanges = [VyntraDsInputSmoke]::ChangedPixels($pressed, $releasedAfter)
  if ($pressChanges -lt 5000 -or $releaseChanges -lt 5000) {
    throw "Player SDL did not show the expected DS input response (press=$pressChanges, release=$releaseChanges pixels)"
  }

  [void][VyntraDsInputSmoke]::SendMessage($window, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
  if (-not $process.WaitForExit(15000)) { throw "Player did not close after WM_CLOSE" }
  [PSCustomObject]@{
    result = "PASS"
    input = "Z → libretro A → DS KEYINPUT"
    changed_pixels_on_press = $pressChanges
    changed_pixels_on_release = $releaseChanges
    rom_sha256 = (Get-FileHash -LiteralPath $rom -Algorithm SHA256).Hash.ToLowerInvariant()
    core_sha256 = (Get-FileHash -LiteralPath (Join-Path $cores "pokedaisy_ds_libretro.dll") -Algorithm SHA256).Hash.ToLowerInvariant()
    data_root = $data
  }
}
finally {
  if (-not $process.HasExited) {
    [void][VyntraDsInputSmoke]::SendMessage($process.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
    if (-not $process.WaitForExit(5000)) { $process.Kill() }
  }
  $process.Dispose()
}
