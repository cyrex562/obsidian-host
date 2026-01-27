$ErrorActionPreference = "Stop"

Write-Host "STEP 1: Building Frontend..." -ForegroundColor Green
Push-Location frontend
if (-not (Test-Path "node_modules")) {
    Write-Host "Installing frontend dependencies..."
    npm install
}
Write-Host "Compiling TypeScript..."
npm run build:simple
Pop-Location

Write-Host "STEP 2: Building Backend (Release)..." -ForegroundColor Green
cargo build --release

Write-Host "STEP 3: Creating Distribution Bundle..." -ForegroundColor Green
$distDir = "dist"
if (Test-Path $distDir) {
    Remove-Item -Recurse -Force $distDir
}
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

Write-Host "Copying Binary..."
Copy-Item "target/release/obsidian-host.exe" "$distDir/obsidian-host.exe"

# Write-Host "Copying Frontend Assets... (Skipped, assets are embedded)"
# $publicDir = "$distDir/frontend/public"
# New-Item -ItemType Directory -Force -Path $publicDir | Out-Null
# Copy-Item -Recurse "frontend/public/*" $publicDir

Write-Host "Copying Configuration..."
Copy-Item "config.toml" "$distDir/config.toml"

Write-Host "Build Complete!" -ForegroundColor Cyan
Write-Host "Artifacts are available in the '$distDir' directory."
Write-Host "To run: cd $distDir; ./obsidian-host.exe"
