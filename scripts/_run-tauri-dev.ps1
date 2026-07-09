$ErrorActionPreference = 'Continue'
Set-Location -LiteralPath 'F:\Grok Gui\grok-desktop'
$log = Join-Path (Get-Location) 'tauri-dev.log'
Write-Host '=== Grok Desktop development ===' -ForegroundColor Green
Write-Host 'Vite must serve http://127.0.0.1:1420 then the app window opens.' -ForegroundColor DarkGray
Write-Host "Log: $log" -ForegroundColor DarkGray
Write-Host ''
npm run tauri:dev 2>&1 | Tee-Object -FilePath $log
Write-Host ''
Write-Host 'tauri:dev exited. Press any key to close this window...' -ForegroundColor Red
try { $null = $Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown') } catch { Start-Sleep 30 }
