# Development launch: Vite + tauri dev together, with honest status.
# Never claims success until http://127.0.0.1:1420 responds.
# ASCII-only for Windows PowerShell 5.1.

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Root "package.json"))) {
  $Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

Set-Location -LiteralPath $Root
Write-Host ""
Write-Host "=== Grok Desktop development launch ===" -ForegroundColor Cyan

function Write-Status([string]$msg, [string]$color = "White") {
  $ts = Get-Date -Format "HH:mm:ss"
  Write-Host "[$ts] $msg" -ForegroundColor $color
}

function Stop-PortListeners([int]$Port) {
  try {
    $conns = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
  } catch { $conns = @() }
  foreach ($c in @($conns)) {
    $procId = $c.OwningProcess
    if ($procId -and $procId -ne 0) {
      try {
        $proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
        if ($proc) {
          Write-Status "Freeing port $Port (PID $procId $($proc.ProcessName))" "Yellow"
          Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
        }
      } catch {}
    }
  }
}

function Test-DevServer {
  try {
    $resp = Invoke-WebRequest -Uri "http://127.0.0.1:1420/" -UseBasicParsing -TimeoutSec 2
    return ($resp.StatusCode -ge 200 -and $resp.StatusCode -lt 500)
  } catch {
    return $false
  }
}

Write-Status "Stopping stale grok-desktop / port 1420 holders..." "Yellow"
Get-Process -Name "grok-desktop" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Stop-PortListeners 1420
Stop-PortListeners 1421
Start-Sleep -Milliseconds 400

if (-not (Test-Path (Join-Path $Root "node_modules"))) {
  Write-Status "npm install..." "Yellow"
  npm install
  if ($LASTEXITCODE -ne 0) { Write-Status "FAILED npm install" "Red"; exit 1 }
}

$log = Join-Path $Root "tauri-dev.log"
if (Test-Path $log) { Remove-Item $log -Force -ErrorAction SilentlyContinue }

$runner = Join-Path $PSScriptRoot "_run-tauri-dev.ps1"
@"
`$ErrorActionPreference = 'Continue'
Set-Location -LiteralPath '$Root'
`$log = Join-Path (Get-Location) 'tauri-dev.log'
Write-Host 'Starting dev server + app (tauri:dev)...' -ForegroundColor Green
Write-Host 'Expected Vite: http://127.0.0.1:1420' -ForegroundColor DarkGray
npm run tauri:dev 2>&1 | Tee-Object -FilePath `$log
Write-Host 'tauri:dev exited.' -ForegroundColor Red
try { `$null = `$Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown') } catch { Start-Sleep 60 }
"@ | Set-Content -LiteralPath $runner -Encoding ASCII

Write-Status "Starting dev server (Vite on 127.0.0.1:1420) + Tauri..." "Green"
$proc = Start-Process -FilePath "powershell.exe" -WorkingDirectory $Root -PassThru -ArgumentList @(
  "-NoExit", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $runner
)

Write-Status "Waiting for connection to http://127.0.0.1:1420 ..." "Cyan"
$deadline = (Get-Date).AddSeconds(120)
$ready = $false
$attempt = 0
while ((Get-Date) -lt $deadline) {
  $attempt++
  if (Test-DevServer) {
    $ready = $true
    break
  }
  if ($proc.HasExited) {
    Write-Status "FAILED: launcher console exited early" "Red"
    if (Test-Path $log) { Get-Content $log -Tail 40 }
    Write-Status "Connection failed - not starting a bare exe." "Red"
    exit 1
  }
  Write-Status "Connection failed - retrying... (attempt $attempt)" "Yellow"
  Start-Sleep -Seconds 2
}

if (-not $ready) {
  Write-Status "FAILED: dev server never became reachable on 127.0.0.1:1420" "Red"
  Write-Status "Fix: free port 1420, run npm run dev manually, check tauri-dev.log" "Yellow"
  if (Test-Path $log) { Get-Content $log -Tail 50 }
  exit 1
}

Write-Status "Dev server is UP on http://127.0.0.1:1420" "Green"
Write-Status "Waiting for grok-desktop window (Rust compile may still be running)..." "Cyan"

$appDeadline = (Get-Date).AddSeconds(180)
$appOk = $false
while ((Get-Date) -lt $appDeadline) {
  # Dev server must stay up - if it dies, do not claim success
  if (-not (Test-DevServer)) {
    Write-Status "FAILED: dev server died while waiting for the app window" "Red"
    exit 1
  }
  $app = Get-Process -Name "grok-desktop" -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($app -and $app.MainWindowHandle -ne 0) {
    $appOk = $true
    Write-Status "App window: '$($app.MainWindowTitle)' PID=$($app.Id)" "Green"
    break
  }
  Write-Status "Waiting for app window..." "DarkGray"
  Start-Sleep -Seconds 3
}

if (-not $appOk) {
  Write-Status "PARTIAL: dev server is up, but app window not detected yet." "Yellow"
  Write-Status "Watch the console window; first Rust compile can take several minutes." "Yellow"
  Write-Status "NOT claiming full success until the Grok Desktop UI appears." "Yellow"
  exit 0
}

# Final honesty check: both server and process
if ((Test-DevServer) -and (Get-Process -Name "grok-desktop" -ErrorAction SilentlyContinue)) {
  Write-Status "Dev launch looks healthy (Vite + app process)." "Green"
  Write-Status "If the WebView still shows an error page, click Retry or re-run npm run start:dev." "DarkGray"
  exit 0
}

Write-Status "FAILED: post-check did not confirm Vite + app" "Red"
exit 1
