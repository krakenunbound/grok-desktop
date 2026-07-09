# STABLE launch: Tauri production binary + offline UI from build\
# Raw cargo builds can still use Tauri cfg(dev); use Tauri CLI for embedded assets.
# ASCII-only.

$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Root "package.json"))) {
  $Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
Set-Location -LiteralPath $Root

function S([string]$m, [string]$c = "White") {
  Write-Host ("[{0}] {1}" -f (Get-Date -Format "HH:mm:ss"), $m) -ForegroundColor $c
}

Write-Host ""
Write-Host "=== Grok Desktop STABLE (RELEASE / offline) ===" -ForegroundColor Cyan
S "Root: $Root" "DarkGray"

S "Stopping old grok-desktop..." "Yellow"
Get-Process -Name "grok-desktop" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

if (-not (Test-Path "node_modules")) {
  S "npm install..." "Yellow"
  npm install
  if ($LASTEXITCODE -ne 0) { S "FAILED npm install" "Red"; exit 1 }
}

S "Building production app with Tauri CLI (embeds build\\)..." "Green"
S "This can take several minutes the first time. Do NOT close this window." "Yellow"
Remove-Item Env:TAURI_DEV -ErrorAction SilentlyContinue
Remove-Item Env:TAURI_ENV_DEBUG -ErrorAction SilentlyContinue
npx tauri build --no-bundle --ci
if ($LASTEXITCODE -ne 0) { S "FAILED npx tauri build --no-bundle --ci" "Red"; exit 1 }
if (-not (Test-Path "build\index.html")) { S "FAILED no build\index.html" "Red"; exit 1 }

$exe = Join-Path $Root "src-tauri\target\release\grok-desktop.exe"
if (-not (Test-Path $exe)) { S "FAILED missing $exe" "Red"; exit 1 }
S "Release binary ready" "Green"

# Detached start so shell wrappers cannot kill the app on exit
S "Starting RELEASE app (no Vite, no port 1420)..." "Green"
$wd = Split-Path $exe
# cmd start detaches from this PowerShell job object
cmd.exe /c "start `"`" /D `"$wd`" `"$exe`""

Start-Sleep -Seconds 4
$proc = Get-Process -Name "grok-desktop" -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) {
  S "FAILED: grok-desktop process not found after start" "Red"
  exit 1
}

S ("Process PID={0} Title='{1}' Handle={2}" -f $proc.Id, $proc.MainWindowTitle, $proc.MainWindowHandle) "Green"
try {
  Invoke-WebRequest "http://127.0.0.1:1420/" -UseBasicParsing -TimeoutSec 1 | Out-Null
  S "Note: something is on port 1420 (not required for release)" "DarkGray"
} catch {
  S "Port 1420 is free (correct for offline release)" "DarkGray"
}

S "SUCCESS criteria: you should see the Grok Desktop UI (not a browser error page)." "Cyan"
S "If you still see ERR_CONNECTION_REFUSED, you opened a DEBUG exe - close it and re-run this bat." "Yellow"
exit 0
