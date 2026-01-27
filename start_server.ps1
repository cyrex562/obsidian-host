# Start Obsidian Host server in background
# Kills any existing instances and starts new one as detached process

Write-Host "Stopping any running obsidian-host processes..." -ForegroundColor Yellow
Get-Process | Where-Object { $_.ProcessName -eq "obsidian-host" } | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1

Write-Host "Building obsidian-host..." -ForegroundColor Cyan
& cargo build 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

Write-Host "Starting obsidian-host server in debug mode..." -ForegroundColor Cyan

# Start the executable directly as a background process
$exePath = ".\target\debug\obsidian-host.exe"
$process = Start-Process -FilePath $exePath -WorkingDirectory $PSScriptRoot -PassThru -WindowStyle Hidden

Write-Host "Server started with PID: $($process.Id)" -ForegroundColor Green

# Wait for server to be ready
Write-Host "Waiting for server to listen on port 8080..." -ForegroundColor Cyan
$maxAttempts = 20
$attempt = 0
$ready = $false

while ($attempt -lt $maxAttempts -and -not $ready) {
    Start-Sleep -Seconds 1
    $attempt++
    
    $listening = Get-NetTCPConnection -LocalPort 8080 -State Listen -ErrorAction SilentlyContinue
    if ($listening) {
        $ready = $true
        Write-Host "Server is ready on port 8080!" -ForegroundColor Green
    }
    else {
        Write-Host "." -NoNewline
    }
}

if (-not $ready) {
    Write-Host "`nServer did not start within 20 seconds. Check logs in .\logs\" -ForegroundColor Red
    exit 1
}

Write-Host "`nServer is running in background. Use 'Get-Process obsidian-host' to check status." -ForegroundColor Green
Write-Host "To stop: Get-Process obsidian-host | Stop-Process -Force" -ForegroundColor Yellow
