# Python TUI Deployment Tool

## Overview

`scripts/deploy_tui.py` provides an interactive terminal workflow and CLI commands for:

- building the Vue frontend
- building the embedded `obsidian-host` backend release for a Linux VM
- assembling a deterministic `dist/` bundle
- packaging desktop placeholders with a template config file
- deploying idempotently to a target defined in `targets.toml`
- installing/updating a `systemd` service on the remote host

## Current Scope

The repo does **not** currently contain a native desktop executable crate. Because of that, the tool creates a desktop placeholder bundle under `dist/desktop/` instead of building a desktop binary.

## Requirements

Local machine:

- Python 3.11+
- Node.js / npm
- Rust toolchain
- `cross` on non-Linux hosts for the default local Linux server build (containerized via Docker)
- `ssh` and `scp`

For local builds on Windows/macOS using `cross`, Docker Desktop must be running and the Linux daemon must be reachable.

Remote VM:

- Linux with `systemd`
- target user must have `sudo` access for writing the systemd unit and managing the service

## Targets File

The tool reads deployment targets from `targets.toml`.

Example:

```toml
[obsidian-test-a]
ssh_host = "obsidian-test-a"
ssh_user = "box-admin"
ssh_port = 22
ip_address = "100.123.8.84"
http_port = 8080
app_dir = "/opt/obsidian-host"
```

## Dist Layout

The build step recreates `dist/` and writes:

- `dist/server/obsidian-host`
- `dist/config.template.toml`
- `dist/server.config.example.toml`
- `dist/manifest.json`
- `dist/deploy/obsidian-host-<release_id>.tar.gz`
- `dist/desktop/config.template.toml`
- `dist/desktop/README.txt`

## Interactive Usage

Run with no subcommand to open the menu:

```powershell
python .\scripts\deploy_tui.py
```

## CLI Usage

List targets:

```powershell
python .\scripts\deploy_tui.py targets
```

Build artifacts:

```powershell
python .\scripts\deploy_tui.py build --target obsidian-test-a
```

Build artifacts using remote host compilation explicitly:

```powershell
python .\scripts\deploy_tui.py build --target obsidian-test-a --remote-build
```

Deploy an existing `dist/` bundle:

```powershell
python .\scripts\deploy_tui.py deploy --target obsidian-test-a
```

Build and deploy in one step:

```powershell
python .\scripts\deploy_tui.py build-and-deploy --target obsidian-test-a
```

Build and deploy using remote host compilation explicitly:

```powershell
python .\scripts\deploy_tui.py build-and-deploy --target obsidian-test-a --remote-build
```

## Remote Layout

The script manages these remote paths under `app_dir`:

- `releases/` — versioned release payloads
- `shared/` — mutable runtime files (`config.toml`, SQLite DB, logs)
- `tmp/` — temporary upload staging
- `current` — symlink to the active release

The generated `systemd` unit runs the server with:

- `WorkingDirectory={app_dir}/shared`
- `ExecStart={app_dir}/current/obsidian-host`

That keeps `config.toml`, `obsidian-host.db`, and `logs/` in the persistent shared directory.

## Idempotence Rules

- release tarball upload is skipped if the exact release already exists remotely
- remote `config.toml` is created only if missing
- the `systemd` unit is updated only if the rendered unit file changed
- the service restarts only when the unit, config creation, or active release changed
- a health check is run after deployment

## Notes

- The tool uses `npm run build`, not the stale `build:simple` reference found in older docs/scripts.
- The default build strategy is now **local** on all platforms.
- On Windows and other non-Linux hosts, the local Linux server build uses `cross` (containerized on the local machine) rather than compiling on the remote VM.
- Remote compilation is still available, but only when `--remote-build` is passed explicitly.
- The deploy tool now performs an early Docker daemon readiness check for local `cross` builds and fails fast with guidance if Docker is not reachable.
