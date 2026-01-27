# Build and Deployment Guide

## Prerequisites

- **Rust**: Latest stable version (install via rustup)
- **Node.js**: LTS version (for frontend)
- **PowerShell**: For running build scripts (Windows)

## Building for Release

We have provided a PowerShell script to automate the build process, which includes building the frontend assets and the backend binary in release mode.

### Windows

Run the following command from the project root:

```powershell
.\scripts\build_release.ps1
```

This will:
1.  Install frontend dependencies (if missing).
2.  Compile TypeScript frontend code.
3.  Build the Rust backend with optimizations (LTO, stripping symbols) and **embedded frontend assets**.
4.  Create a `dist` directory containing:
    -   `obsidian-host.exe` (Standalone binary with UI included)
    -   `config.toml`

To run the release build:

```powershell
cd dist
.\obsidian-host.exe
```

## Manual Build Steps

If you cannot use the script, follow these steps:

1.  **Build Frontend**:
    ```bash
    cd frontend
    npm install
    npm run build:simple
    cd ..
    ```

2.  **Build Backend**:
    *Ensure `frontend/public` is populated first, as it is embedded at compile time.*
    ```bash
    cargo build --release
    ```

3.  **Assemble**:
    -   Create a directory (e.g., `dist`).
    -   Copy `target/release/obsidian-host` (or `.exe`) to `dist`.
    -   Copy `config.toml` to `dist`.
    -   *(Optional)* You do **not** need to copy `frontend/public` as it is inside the binary.

## Binary Optimization

The `Cargo.toml` is configured with a custom release profile to minimize binary size:

-   `opt-level = "z"`: Optimize for size.
-   `lto = true`: Link Time Optimization enabled.
-   `codegen-units = 1`:  Maximize optimization quality at cost of compile time.
-   `panic = "abort"`: Removes stack unwinding code.
-   `strip = true`: Removes debugging symbols.

## Cross-Compilation

To cross-compile for other platforms (e.g., Linux, macOS), we recommend using [`cross`](https://github.com/cross-rs/cross).

1.  **Install cross**:
    ```bash
    cargo install cross
    ```

2.  **Build for Linux (x86_64)**:
    ```bash
    cross build --target x86_64-unknown-linux-gnu --release
    ```
    *Note: This requires Docker to be running.*

3.  **Build for Windows (from Linux/macOS)**:
    ```bash
    cross build --target x86_64-pc-windows-gnu --release
    ```

After cross-compiling, follow the "Assemble" steps above, using the binary from `target/<target-triple>/release/`.
