# kizn installer for Windows (PowerShell 5.1 or later).
#
#   irm https://kizn.app/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://kizn.app/install.ps1))) -Version 0.1.0-rc.1
#   & ([scriptblock]::Create((irm https://kizn.app/install.ps1))) -Uninstall [-Purge]
#
# Downloads a release from GitHub, checks its SHA-256 against the release's
# SHA256SUMS, and installs for this user only (no admin):
#   %LOCALAPPDATA%\Programs\kizn\kizn.exe and a Start menu shortcut.
#
# Environment: KIZN_VERSION (same as -Version), KIZN_RELEASE_BASE (a mirror
# of the GitHub releases URL layout; the tests use it).

param(
  [string]$Version = $env:KIZN_VERSION,
  [switch]$Uninstall,
  [switch]$Purge
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# throw, never exit: under `irm | iex` an exit would close the user's
# PowerShell window. Run as a file, an uncaught throw exits non-zero.
function Fail([string]$message) {
  throw "kizn install: $message"
}

if ($Purge -and -not $Uninstall) { Fail '-Purge only goes with -Uninstall' }

$base = if ($env:KIZN_RELEASE_BASE) { $env:KIZN_RELEASE_BASE } else { 'https://github.com/robertjdominguez/kizn/releases' }
if ($Version) { $Version = $Version.TrimStart('v') }

$arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
if ($arch -ne 'AMD64') { Fail "kizn has no Windows $arch build yet (x86_64 only)" }

$asset = 'kizn-windows-x86_64.zip'
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\kizn'
$exe = Join-Path $installDir 'kizn.exe'
$shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\kizn.lnk'
$appData = Join-Path $env:APPDATA 'kizn'
$dataDir = Join-Path $appData 'data'
$configDir = Join-Path $appData 'config'

# ---- uninstall ---------------------------------------------------------------

if ($Uninstall) {
  if (Test-Path $installDir) {
    Remove-Item -Recurse -Force $installDir
    Write-Host "Removed $installDir"
  } else {
    Write-Host "kizn isn't installed at $installDir"
  }
  if (Test-Path $shortcut) {
    Remove-Item -Force $shortcut
    Write-Host "Removed $shortcut"
  }
  if ($Purge) {
    foreach ($d in @($dataDir, $configDir)) {
      if (Test-Path $d) {
        Remove-Item -Recurse -Force $d
        Write-Host "Deleted your kizn data: $d"
      }
    }
    if ((Test-Path $appData) -and -not (Get-ChildItem $appData -Force)) { Remove-Item -Force $appData }
  } else {
    Write-Host "Your kizn data is still in $dataDir"
    Write-Host "and your config in $configDir"
    Write-Host 'To delete it too: -Uninstall -Purge'
  }
  return
}

# ---- download and verify -----------------------------------------------------

[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$url = if ($Version) { "$base/download/v$Version" } else { "$base/latest/download" }
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("kizn-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
  function Get-Asset([string]$name) {
    try {
      Invoke-WebRequest -UseBasicParsing -Uri "$url/$name" -OutFile (Join-Path $tmp $name)
    } catch {
      $hint = if ($Version) { " (is v$Version a kizn release?)" } else { '' }
      Fail "couldn't download $url/$name$hint"
    }
  }

  Get-Asset 'SHA256SUMS'
  $sums = @{}
  foreach ($line in Get-Content (Join-Path $tmp 'SHA256SUMS')) {
    $parts = $line -split '\s+', 2
    if ($parts.Count -eq 2) { $sums[$parts[1].TrimStart('*')] = $parts[0].ToLowerInvariant() }
  }

  foreach ($name in @('VERSION', $asset)) {
    Get-Asset $name
    $got = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $name)).Hash.ToLowerInvariant()
    $want = $sums[$name]
    if (-not $want -or $want -ne $got) {
      $shown = if ($want) { $want } else { 'no entry' }
      Fail "the download failed verification: $name doesn't match the release's SHA256SUMS (expected $shown, got $got). Nothing was installed."
    }
  }
  Write-Host 'checksum verified'
  $installed = (Get-Content (Join-Path $tmp 'VERSION') -Raw).Trim()
  Write-Host "Installing kizn v$installed"

  # ---- install ---------------------------------------------------------------

  $unpacked = Join-Path $tmp 'unpacked'
  Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath $unpacked
  if (-not (Test-Path (Join-Path $unpacked 'kizn.exe'))) { Fail "the release's Windows archive has no kizn.exe" }
  if (Test-Path $installDir) { Remove-Item -Recurse -Force $installDir }
  New-Item -ItemType Directory -Path (Split-Path $installDir) -Force | Out-Null
  Move-Item $unpacked $installDir

  New-Item -ItemType Directory -Path (Split-Path $shortcut) -Force | Out-Null
  $link = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
  $link.TargetPath = $exe
  $link.WorkingDirectory = $installDir
  $link.Description = 'Learn something properly'
  $link.Save()
} finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host "kizn v$installed is installed: $exe"
Write-Host "It's in your Start menu as kizn."
Write-Host "Your data will live in $dataDir"
Write-Host "and your config in $configDir"

$webview = @(
  'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
  'HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
) | Where-Object { Test-Path $_ }
if (-not $webview) {
  Write-Host 'Note: kizn needs the Microsoft Edge WebView2 Runtime, which this machine may not have:'
  Write-Host '      https://developer.microsoft.com/microsoft-edge/webview2/'
}
