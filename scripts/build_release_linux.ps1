#!/usr/bin/env pwsh
# Build a Linux x86_64 release of both the server and the desktop client.
#
# Prerequisites
# -------------
# 1. Docker must be running (used by `cross` for the sysroot).
# 2. Install `cross`:
#      cargo install cross
# 3. Add the Rust target (optional – cross does this automatically via Docker,
#    but handy for IDE tooling):
#      rustup target add x86_64-unknown-linux-gnu
#
# Usage
# -----
#   ./scripts/build_release_linux.ps1              # builds both binaries
#   ./scripts/build_release_linux.ps1 -ServerOnly  # build server only
#   ./scripts/build_release_linux.ps1 -DesktopOnly # build desktop only
#
# Output
# ------
#   dist-linux/obsidian-host          (server)
#   dist-linux/obsidian-desktop       (desktop GUI – requires a display/Wayland/X11 on target)

param(
    [switch]$ServerOnly,
    [switch]$DesktopOnly
)

$ErrorActionPreference = "Stop"

$TARGET = "x86_64-unknown-linux-gnu"
$DIST = "dist-linux"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
function Invoke-Cross {
    param([string]$Package)
    Write-Host "  cross build --release -p $Package --target $TARGET" -ForegroundColor DarkGray
    cross build --release -p $Package --target $TARGET
    if ($LASTEXITCODE -ne 0) { throw "cross build failed for $Package" }
}

function Copy-Artifact {
    param([string]$SourceName, [string]$DestName)
    $src = "target/$TARGET/release/$SourceName"
    if (-not (Test-Path $src)) {
        throw "Expected artifact not found: $src"
    }
    Copy-Item $src "$DIST/$DestName"
    Write-Host "  Copied $src -> $DIST/$DestName"
}

# ---------------------------------------------------------------------------
# Check cross is available
# ---------------------------------------------------------------------------
if (-not (Get-Command cross -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: `cross` is not installed.  Run: cargo install cross" -ForegroundColor Red
    Write-Host "Then make sure Docker is running before re-running this script." -ForegroundColor Yellow
    exit 1
}

# ---------------------------------------------------------------------------
# Prepare output directory
# ---------------------------------------------------------------------------
if (Test-Path $DIST) { Remove-Item -Recurse -Force $DIST }
New-Item -ItemType Directory -Force -Path $DIST | Out-Null

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
if (-not $DesktopOnly) {
    Write-Host "`nSTEP: Building obsidian-server for $TARGET ..." -ForegroundColor Green
    Invoke-Cross -Package "obsidian-host"
    Copy-Artifact -SourceName "obsidian-host" -DestName "obsidian-host"
}

if (-not $ServerOnly) {
    Write-Host "`nSTEP: Building obsidian-desktop for $TARGET ..." -ForegroundColor Green
    # NOTE: The desktop binary links against Wayland/X11 libraries at runtime on Linux.
    # It will NOT run on a headless server without a display or a virtual framebuffer
    # (e.g.  Xvfb, Weston, or a VNC session).
    Invoke-Cross -Package "obsidian-desktop"
    Copy-Artifact -SourceName "obsidian-desktop" -DestName "obsidian-desktop"
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
Write-Host "`nBuild complete!  Artifacts in ./$DIST/" -ForegroundColor Cyan
Get-ChildItem $DIST | Format-Table Name, Length, LastWriteTime

Write-Host @"

To deploy the server to a Linux host:
  scp dist-linux/obsidian-host  user@host:/opt/obsidian/
  scp config.toml               user@host:/opt/obsidian/
  ssh user@host '/opt/obsidian/obsidian-host'

To run the desktop client on Linux (requires display):
  scp dist-linux/obsidian-desktop  user@desktop:/usr/local/bin/
  ssh -X user@desktop obsidian-desktop     # X11 forwarding
  # or: DISPLAY=:0 obsidian-desktop        # local desktop session

Notes on CI (GitHub Actions) \u2014 alternative to cross:
  jobs:
    build-linux:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: dtolnay/rust-toolchain@stable
        - run: cargo build --release -p obsidian-host -p obsidian-desktop
        - uses: actions/upload-artifact@v4
          with:
            name: linux-x86_64
            path: target/release/obsidian-*
"@
