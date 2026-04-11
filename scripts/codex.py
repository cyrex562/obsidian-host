#!/usr/bin/env python3
"""
codex — unified CLI for building, deploying, configuring, and testing Codex.

Usage:
    python scripts/codex.py [COMMAND] [OPTIONS]
    ./scripts/codex.py build --server --release
    ./scripts/codex.py deploy codex-test-a --build-first
    ./scripts/codex.py test --all
    ./scripts/codex.py configure --target codex-test-a --show
    ./scripts/codex.py doctor --target codex-test-a
"""
from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import secrets
import shutil
import string
import subprocess
import tarfile
import tempfile
import textwrap
import time
import urllib.error
import urllib.request
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path

import click
from rich.columns import Columns
from rich.console import Console
from rich.panel import Panel
from rich.progress import Progress, SpinnerColumn, TextColumn, TimeElapsedColumn
from rich.table import Table
from rich import box

try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ModuleNotFoundError:
        raise SystemExit(
            "Python 3.11+ required, or install tomli: pip install tomli"
        )

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

REPO_ROOT = Path(__file__).resolve().parents[1]
TARGETS_FILE = REPO_ROOT / "targets.toml"
CONFIG_TEMPLATE = REPO_ROOT / "config.toml"
SYSTEMD_TEMPLATE = REPO_ROOT / "deploy" / "systemd" / "codex.service.template"
DIST_DIR = REPO_ROOT / "dist"
FRONTEND_DIR = REPO_ROOT / "frontend"
PLUGINS_DIR = REPO_ROOT / "plugins"
FRONTEND_NODE_MODULES = FRONTEND_DIR / "node_modules"
FRONTEND_BUILD_OUTPUT = REPO_ROOT / "target" / "frontend"
LINUX_TARGET = "x86_64-unknown-linux-gnu"
SERVICE_NAME = "codex"
BIN_NAME = "codex"
DESKTOP_BIN_NAME = "codex-desktop"

console = Console()


# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------


class CodexError(RuntimeError):
    pass


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class DeploymentTarget:
    name: str
    ssh_host: str
    ssh_user: str
    ssh_port: int
    ip_address: str
    http_port: int
    app_dir: str

    @property
    def ssh_destination(self) -> str:
        return f"{self.ssh_user}@{self.ssh_host}"

    @property
    def releases_dir(self) -> str:
        return f"{self.app_dir}/releases"

    @property
    def shared_dir(self) -> str:
        return f"{self.app_dir}/shared"

    @property
    def current_dir(self) -> str:
        return f"{self.app_dir}/current"

    @property
    def tmp_dir(self) -> str:
        return f"{self.app_dir}/tmp"

    @property
    def remote_config_path(self) -> str:
        return f"{self.shared_dir}/config.toml"

    @property
    def systemd_unit_path(self) -> str:
        return f"/etc/systemd/system/{SERVICE_NAME}.service"


@dataclass
class BuildArtifacts:
    target_name: str
    server_binary: Path
    server_sha256: str
    dist_dir: Path
    manifest_path: Path
    tarball_path: Path
    release_id: str
    git_commit: str


@dataclass
class DeployResult:
    release_uploaded: bool
    release_activated: bool
    config_created: bool
    config_updated: bool
    unit_updated: bool
    service_restarted: bool
    service_enabled: bool
    healthcheck_ok: bool
    generated_admin_username: str | None = None
    generated_admin_password: str | None = None
    bootstrap_status: str | None = None  # "created", "skipped", or None (not checked)


@dataclass
class PreflightCheck:
    name: str
    ok: bool
    detail: str


# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------


def step(title: str) -> None:
    console.rule(f"[bold cyan]{title}[/bold cyan]", style="cyan")


def ok(message: str) -> None:
    console.print(f"  [bold green]✓[/bold green]  {message}")


def info(message: str) -> None:
    console.print(f"  [dim]→[/dim]  {message}")


def warn(message: str) -> None:
    console.print(f"  [bold yellow]⚠[/bold yellow]  {message}")


def fail(message: str) -> None:
    console.print(f"  [bold red]✗[/bold red]  {message}")


# ---------------------------------------------------------------------------
# Shell helpers
# ---------------------------------------------------------------------------


def run_cmd(
    command: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    capture: bool = False,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    resolved = shutil.which(command[0])
    if resolved:
        command = [resolved] + command[1:]
    info(f"[dim]{' '.join(command)}[/dim]")
    result = subprocess.run(
        command,
        cwd=str(cwd) if cwd else None,
        env=env,
        text=True,
        check=False,
        capture_output=capture,
    )
    if check and result.returncode != 0:
        detail = (result.stderr or result.stdout or "").strip()
        raise CodexError(detail or f"Command exited {result.returncode}: {' '.join(command)}")
    return result


def shell_quote(value: str) -> str:
    return "'" + value.replace("'", "'\"'\"'") + "'"


def ssh_cmd(
    target: DeploymentTarget,
    command: str,
    *,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    return run_cmd(
        ["ssh", "-p", str(target.ssh_port), target.ssh_destination, f"bash -lc {shell_quote(command)}"],
        cwd=REPO_ROOT,
        capture=capture,
    )


def scp_upload(target: DeploymentTarget, local: Path, remote: str) -> None:
    run_cmd(
        ["scp", "-P", str(target.ssh_port), str(local), f"{target.ssh_destination}:{remote}"],
        cwd=REPO_ROOT,
    )


def ensure_tool(name: str) -> None:
    if shutil.which(name) is None:
        raise CodexError(f"Required tool '{name}' not found on PATH")


# ---------------------------------------------------------------------------
# Targets
# ---------------------------------------------------------------------------


def load_targets(targets_file: Path = TARGETS_FILE) -> dict[str, DeploymentTarget]:
    if not targets_file.exists():
        raise CodexError(f"Targets file not found: {targets_file}")
    data = tomllib.loads(targets_file.read_text(encoding="utf-8"))
    return {
        name: DeploymentTarget(
            name=name,
            ssh_host=raw["ssh_host"],
            ssh_user=raw["ssh_user"],
            ssh_port=int(raw.get("ssh_port", 22)),
            ip_address=raw["ip_address"],
            http_port=int(raw.get("http_port", 8080)),
            app_dir=raw["app_dir"],
        )
        for name, raw in data.items()
    }


def pick_target(
    name: str | None,
    targets: dict[str, DeploymentTarget],
) -> DeploymentTarget:
    if name:
        if name not in targets:
            raise CodexError(f"Unknown target '{name}'. Run `codex.py targets` to list.")
        return targets[name]

    names = list(targets)
    if len(names) == 1:
        return targets[names[0]]

    table = Table(title="Deployment targets", box=box.SIMPLE)
    table.add_column("#", style="bold cyan", width=4)
    table.add_column("Name")
    table.add_column("Host")
    table.add_column("URL")
    for i, (n, t) in enumerate(targets.items(), 1):
        table.add_row(str(i), n, f"{t.ssh_user}@{t.ssh_host}:{t.ssh_port}", f"http://{t.ip_address}:{t.http_port}")
    console.print(table)

    choice = click.prompt("Select target number", type=click.IntRange(1, len(names)))
    return targets[names[choice - 1]]


# ---------------------------------------------------------------------------
# Git
# ---------------------------------------------------------------------------


def git_commit() -> str:
    try:
        r = run_cmd(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, capture=True)
        return r.stdout.strip()
    except CodexError:
        return "unknown"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------


def build_frontend() -> None:
    ensure_tool("npm")
    step("Building frontend")
    if not FRONTEND_NODE_MODULES.exists():
        run_cmd(["npm", "install"], cwd=FRONTEND_DIR)
    run_cmd(["npm", "run", "build"], cwd=FRONTEND_DIR)
    if not (FRONTEND_BUILD_OUTPUT / "index.html").exists():
        raise CodexError(f"Frontend build output missing: {FRONTEND_BUILD_OUTPUT}")
    ok(f"Frontend built → {FRONTEND_BUILD_OUTPUT}")


def build_server(
    *,
    release: bool = True,
    cross_target: str | None = None,
) -> Path:
    step("Building server")
    profile_flag = ["--release"] if release else []
    profile_dir = "release" if release else "debug"

    # Force re-embed of frontend assets
    run_cmd(["cargo", "clean", "-p", "codex-server"], cwd=REPO_ROOT)

    is_native_linux = platform.system().lower() == "linux"
    effective_target = cross_target or (LINUX_TARGET if not is_native_linux else None)

    if effective_target and not (is_native_linux and effective_target == LINUX_TARGET):
        ensure_tool("cross")
        _ensure_docker()
        run_cmd(
            ["cross", "build", "--target", effective_target, *profile_flag, "-p", "codex-server"],
            cwd=REPO_ROOT,
        )
        binary = REPO_ROOT / "target" / effective_target / profile_dir / BIN_NAME
    else:
        run_cmd(
            ["cargo", "build", *profile_flag, "-p", "codex-server"],
            cwd=REPO_ROOT,
        )
        binary = REPO_ROOT / "target" / profile_dir / BIN_NAME

    if not binary.exists():
        raise CodexError(f"Server binary not found: {binary}")
    ok(f"Server built → {binary}")
    return binary


def build_server_remote(target: DeploymentTarget) -> Path:
    ensure_tool("ssh")
    ensure_tool("scp")
    ensure_remote_layout(target)

    step("Building server on remote host")
    build_id = datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S")
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    archive = DIST_DIR / f"source-{build_id}.tar.gz"
    remote_root = f"{target.tmp_dir}/codex-build-{build_id}"
    remote_archive = f"{remote_root}/source.tar.gz"
    remote_src = f"{remote_root}/src"
    remote_bin = f"{remote_root}/codex"

    _create_source_archive(archive)
    try:
        ssh_cmd(target, f"mkdir -p {shell_quote(remote_root)}")
        scp_upload(target, archive, remote_archive)
        ssh_cmd(
            target,
            textwrap.dedent(f"""
                set -euo pipefail
                export PATH="$HOME/.cargo/bin:$PATH"
                command -v npm || {{ echo "npm not found" >&2; exit 1; }}
                command -v cargo || {{ echo "cargo not found" >&2; exit 1; }}
                rm -rf {shell_quote(remote_src)} && mkdir -p {shell_quote(remote_src)}
                tar -xzf {shell_quote(remote_archive)} -C {shell_quote(remote_src)}
                cd {shell_quote(remote_src)}/frontend && rm -rf node_modules && npm install
                node ./node_modules/vite/bin/vite.js build
                cd {shell_quote(remote_src)}
                cargo build --release -p codex-server
                cp {shell_quote(remote_src + "/target/release/codex")} {shell_quote(remote_bin)}
            """).strip(),
        )
        local_bin = REPO_ROOT / "target" / "remote-build" / target.name / BIN_NAME
        local_bin.parent.mkdir(parents=True, exist_ok=True)
        run_cmd(
            ["scp", "-P", str(target.ssh_port), f"{target.ssh_destination}:{remote_bin}", str(local_bin)],
            cwd=REPO_ROOT,
        )
    finally:
        archive.unlink(missing_ok=True)
        try:
            ssh_cmd(target, f"rm -rf {shell_quote(remote_root)} || true")
        except CodexError:
            pass

    if not local_bin.exists():
        raise CodexError(f"Remote-built binary not found locally: {local_bin}")
    ok(f"Server built remotely → {local_bin}")
    return local_bin


def build_desktop(*, release: bool = True, cross_target: str | None = None) -> Path:
    step("Building desktop")
    profile_flag = ["--release"] if release else []
    profile_dir = "release" if release else "debug"
    is_native_linux = platform.system().lower() == "linux"
    effective_target = cross_target or (LINUX_TARGET if not is_native_linux else None)

    if effective_target and not (is_native_linux and effective_target == LINUX_TARGET):
        ensure_tool("cross")
        _ensure_docker()
        run_cmd(
            ["cross", "build", "--target", effective_target, *profile_flag, "-p", "codex-desktop"],
            cwd=REPO_ROOT,
        )
        binary = REPO_ROOT / "target" / effective_target / profile_dir / DESKTOP_BIN_NAME
    else:
        run_cmd(
            ["cargo", "build", *profile_flag, "-p", "codex-desktop"],
            cwd=REPO_ROOT,
        )
        binary = REPO_ROOT / "target" / profile_dir / DESKTOP_BIN_NAME

    if not binary.exists():
        raise CodexError(f"Desktop binary not found: {binary}")
    ok(f"Desktop built → {binary}")
    return binary


def assemble_dist(
    target: DeploymentTarget,
    *,
    remote_build: bool = False,
    skip_support: bool = False,
    build_desktop_bin: bool = False,
    cross_target: str | None = None,
    release: bool = True,
) -> BuildArtifacts:
    desktop_bin: Path | None = None

    if remote_build:
        server_bin = build_server_remote(target)
    else:
        build_frontend()
        server_bin = build_server(release=release, cross_target=cross_target)
        if build_desktop_bin:
            desktop_bin = build_desktop(release=release, cross_target=cross_target)
        if not skip_support:
            step("Building supporting crates")
            run_cmd(["cargo", "build", "-p", "codex-client"], cwd=REPO_ROOT)
            run_cmd(["cargo", "build", "-p", "codex-types"], cwd=REPO_ROOT)

    step("Assembling dist")
    if DIST_DIR.exists():
        shutil.rmtree(DIST_DIR)
    DIST_DIR.mkdir(parents=True, exist_ok=True)

    server_dir = DIST_DIR / "server"
    server_dir.mkdir()
    bin_copy = server_dir / BIN_NAME
    shutil.copy2(server_bin, bin_copy)
    plugins_copy = server_dir / "plugins"
    shutil.copytree(PLUGINS_DIR, plugins_copy, dirs_exist_ok=True)

    # Copy desktop binary if it was built
    desktop_copy: Path | None = None
    if desktop_bin is not None:
        desktop_dir = DIST_DIR / "desktop"
        desktop_dir.mkdir(parents=True, exist_ok=True)
        desktop_copy = desktop_dir / DESKTOP_BIN_NAME
        shutil.copy2(desktop_bin, desktop_copy)

    shutil.copy2(CONFIG_TEMPLATE, DIST_DIR / "config.template.toml")

    commit = git_commit()
    sha = sha256_file(bin_copy)
    release_id = sha[:16]

    manifest: dict = {
        "build_time_utc": datetime.now(timezone.utc).isoformat(),
        "git_commit": commit,
        "release_id": release_id,
        "target_platform": LINUX_TARGET,
        "deployment_target": target.name,
        "server": {
            "artifact": str(bin_copy.relative_to(DIST_DIR)).replace("\\", "/"),
            "sha256": sha,
            "frontend_embedded": True,
        },
    }
    if desktop_copy is not None:
        manifest["desktop"] = {
            "artifact": str(desktop_copy.relative_to(DIST_DIR)).replace("\\", "/"),
            "sha256": sha256_file(desktop_copy),
        }
    else:
        manifest["desktop"] = {"placeholder": True, "path": "desktop"}

    manifest_path = DIST_DIR / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

    deploy_dir = DIST_DIR / "deploy"
    deploy_dir.mkdir()
    tarball = deploy_dir / f"codex-{release_id}.tar.gz"
    with tarfile.open(tarball, "w:gz") as arc:
        arc.add(bin_copy, arcname=BIN_NAME)
        arc.add(manifest_path, arcname="manifest.json")
        arc.add(plugins_copy, arcname="plugins")
        if desktop_copy is not None:
            arc.add(desktop_copy, arcname=f"desktop/{DESKTOP_BIN_NAME}")

    ok(f"Dist assembled at {DIST_DIR}  [release_id={release_id}]")
    return BuildArtifacts(
        target_name=target.name,
        server_binary=bin_copy,
        server_sha256=sha,
        dist_dir=DIST_DIR,
        manifest_path=manifest_path,
        tarball_path=tarball,
        release_id=release_id,
        git_commit=commit,
    )


def load_existing_artifacts(target: DeploymentTarget) -> BuildArtifacts:
    manifest_path = DIST_DIR / "manifest.json"
    if not manifest_path.exists():
        raise CodexError("dist/manifest.json missing — run `codex.py build` first.")
    m = json.loads(manifest_path.read_text(encoding="utf-8"))
    bin_copy = DIST_DIR / m["server"]["artifact"]
    tarball = DIST_DIR / "deploy" / f"codex-{m['release_id']}.tar.gz"
    for p in [bin_copy, tarball]:
        if not p.exists():
            raise CodexError(f"Expected dist artifact missing: {p}")
    return BuildArtifacts(
        target_name=target.name,
        server_binary=bin_copy,
        server_sha256=m["server"]["sha256"],
        dist_dir=DIST_DIR,
        manifest_path=manifest_path,
        tarball_path=tarball,
        release_id=m["release_id"],
        git_commit=m.get("git_commit", "unknown"),
    )


# ---------------------------------------------------------------------------
# Deploy helpers
# ---------------------------------------------------------------------------


def ensure_remote_layout(target: DeploymentTarget) -> None:
    step("Ensuring remote layout")
    ssh_cmd(
        target,
        " && ".join([
            f"sudo mkdir -p {shell_quote(target.releases_dir)}",
            f"sudo mkdir -p {shell_quote(target.shared_dir)}",
            f"sudo mkdir -p {shell_quote(target.tmp_dir)}",
            f"sudo chown -R {shell_quote(target.ssh_user + ':' + target.ssh_user)} {shell_quote(target.app_dir)}",
        ]),
    )
    ok("Remote directories ready")


def _remote_path_exists(target: DeploymentTarget, path: str) -> bool:
    r = ssh_cmd(target, f"[ -e {shell_quote(path)} ] && echo yes || echo no", capture=True)
    return r.stdout.strip() == "yes"


def _remote_sha(target: DeploymentTarget, path: str) -> str | None:
    r = ssh_cmd(
        target,
        f"[ -f {shell_quote(path)} ] && sha256sum {shell_quote(path)} | awk '{{print $1}}' || true",
        capture=True,
    )
    return r.stdout.strip() or None


def _remote_readlink(target: DeploymentTarget, path: str) -> str:
    r = ssh_cmd(target, f"readlink -f {shell_quote(path)} 2>/dev/null || true", capture=True)
    return r.stdout.strip()


def upload_release(target: DeploymentTarget, artifacts: BuildArtifacts) -> bool:
    release_dir = f"{target.releases_dir}/{artifacts.release_id}"
    release_bin = f"{release_dir}/{BIN_NAME}"
    if _remote_path_exists(target, release_bin):
        info(f"Release {artifacts.release_id} already on remote — skipping upload")
        return False
    step("Uploading release")
    remote_tarball = f"{target.tmp_dir}/{artifacts.tarball_path.name}"
    scp_upload(target, artifacts.tarball_path, remote_tarball)
    ssh_cmd(
        target,
        " && ".join([
            f"mkdir -p {shell_quote(release_dir)}",
            f"tar -xzf {shell_quote(remote_tarball)} -C {shell_quote(release_dir)}",
            f"chmod +x {shell_quote(release_bin)}",
            f"rm -f {shell_quote(remote_tarball)}",
        ]),
    )
    ok(f"Uploaded release {artifacts.release_id}")
    return True


def activate_release(target: DeploymentTarget, artifacts: BuildArtifacts) -> bool:
    release_dir = f"{target.releases_dir}/{artifacts.release_id}"
    current = _remote_readlink(target, target.current_dir)
    if current == release_dir:
        info(f"Release {artifacts.release_id} already active")
        return False
    step("Activating release")
    ssh_cmd(target, f"ln -sfn {shell_quote(release_dir)} {shell_quote(target.current_dir)}")
    ok(f"Activated {artifacts.release_id}")
    return True


def ensure_systemd_unit(target: DeploymentTarget) -> bool:
    if not SYSTEMD_TEMPLATE.exists():
        warn(f"Systemd template not found: {SYSTEMD_TEMPLATE} — skipping")
        return False
    step("Ensuring systemd unit")
    unit = SYSTEMD_TEMPLATE.read_text(encoding="utf-8")
    for placeholder, value in {
        "{{SERVICE_NAME}}": SERVICE_NAME,
        "{{SERVICE_USER}}": target.ssh_user,
        "{{APP_DIR}}": target.app_dir,
        "{{WORKING_DIRECTORY}}": target.shared_dir,
        "{{EXEC_START}}": f"{target.current_dir}/{BIN_NAME}",
        "{{CONFIG_PATH}}": target.remote_config_path,
    }.items():
        unit = unit.replace(placeholder, value)

    local_hash = hashlib.sha256(unit.encode()).hexdigest()
    if _remote_sha(target, target.systemd_unit_path) == local_hash:
        info("Systemd unit already up to date")
        return False

    with tempfile.NamedTemporaryFile("w", suffix=".service", delete=False) as f:
        f.write(unit)
        tmp = Path(f.name)
    try:
        remote_tmp = f"{target.tmp_dir}/{SERVICE_NAME}.service"
        scp_upload(target, tmp, remote_tmp)
        ssh_cmd(target, " && ".join([
            f"sudo mv {shell_quote(remote_tmp)} {shell_quote(target.systemd_unit_path)}",
            f"sudo chmod 644 {shell_quote(target.systemd_unit_path)}",
        ]))
    finally:
        tmp.unlink(missing_ok=True)
    ok("Systemd unit updated")
    return True


def ensure_remote_config(target: DeploymentTarget) -> tuple[bool, bool, str | None, str | None]:
    if _remote_path_exists(target, target.remote_config_path):
        r = ssh_cmd(target, f"cat {shell_quote(target.remote_config_path)}", capture=True)
        updated, changed, gen_user, gen_pass = _patch_bootstrap_creds(r.stdout)
        if not changed:
            info(f"Remote config exists at {target.remote_config_path} — preserving")
            return False, False, None, None
        step("Updating remote config bootstrap credentials")
        _upload_config(target, updated)
        ok(f"Config updated at {target.remote_config_path}")
        if gen_user:
            console.print(Panel(f"[bold]username:[/bold] {gen_user}\n[bold]password:[/bold] {gen_pass}", title="[yellow]Generated admin credentials[/yellow]", border_style="yellow"))
        return False, True, gen_user, gen_pass

    step("Creating remote config")
    username, password = _generate_credentials()
    config_text = _render_config(target, bootstrap_user=username, bootstrap_pass=password)
    _upload_config(target, config_text)
    ok(f"Config created at {target.remote_config_path}")
    console.print(Panel(f"[bold]username:[/bold] {username}\n[bold]password:[/bold] {password}", title="[yellow]Generated admin credentials[/yellow]", border_style="yellow"))
    return True, False, username, password


_SERVICE_CANDIDATES = [SERVICE_NAME, "obsidian-host"]


def detect_service_name(target: DeploymentTarget) -> str:
    """Return the name of the active (or at least installed) systemd service."""
    r = ssh_cmd(
        target,
        " || ".join(
            f"systemctl is-active {shell_quote(s)} >/dev/null 2>&1 && echo {shell_quote(s)}"
            for s in _SERVICE_CANDIDATES
        ),
        capture=True,
    )
    detected = r.stdout.strip()
    if detected in _SERVICE_CANDIDATES:
        return detected
    # Fall back to the first installed unit even if inactive.
    r2 = ssh_cmd(
        target,
        " || ".join(
            f"systemctl cat {shell_quote(s)} >/dev/null 2>&1 && echo {shell_quote(s)}"
            for s in _SERVICE_CANDIDATES
        ),
        capture=True,
    )
    detected2 = r2.stdout.strip()
    return detected2 if detected2 in _SERVICE_CANDIDATES else SERVICE_NAME


def ensure_service(
    target: DeploymentTarget,
    *,
    unit_updated: bool,
    release_activated: bool,
    config_changed: bool,
) -> tuple[bool, bool]:
    step("Ensuring service state")
    svc = detect_service_name(target)
    if unit_updated:
        ssh_cmd(target, "sudo systemctl daemon-reload")
    ssh_cmd(target, f"sudo systemctl is-enabled {shell_quote(svc)} >/dev/null 2>&1 || sudo systemctl enable {shell_quote(svc)}")
    should_restart = unit_updated or release_activated or config_changed
    if should_restart:
        ssh_cmd(target, f"sudo systemctl restart {shell_quote(svc)}")
        ok(f"Service {svc!r} restarted")
        return True, True
    ssh_cmd(target, f"sudo systemctl is-active {shell_quote(svc)} >/dev/null 2>&1 || sudo systemctl start {shell_quote(svc)}")
    ok(f"Service {svc!r} running (no restart needed)")
    return True, False


def healthcheck(target: DeploymentTarget, timeout: int = 30) -> bool:
    step("Health check")
    url = f"http://{target.ip_address}:{target.http_port}/"
    deadline = time.time() + timeout
    with Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), TimeElapsedColumn(), console=console) as progress:
        task = progress.add_task(f"Waiting for {url}", total=None)
        while time.time() < deadline:
            try:
                with urllib.request.urlopen(url, timeout=5) as resp:
                    if resp.status == 200:
                        progress.stop()
                        ok(f"Health check passed: {url}")
                        return True
            except urllib.error.HTTPError as e:
                if e.code in (200, 401, 403):
                    # Server is up; auth-gated root is fine
                    progress.stop()
                    ok(f"Health check passed: {url} (HTTP {e.code})")
                    return True
                time.sleep(2)
            except (urllib.error.URLError, TimeoutError, OSError):
                time.sleep(2)
            progress.advance(task)
    warn(f"Health check timed out: {url}")
    return False


def _check_bootstrap_journal(target: DeploymentTarget, service_name: str) -> str | None:
    """Return 'created', 'skipped', or None if undetermined from recent journal output."""
    r = ssh_cmd(
        target,
        f"sudo journalctl -u {service_name} -n 60 --no-pager --output=short 2>/dev/null || true",
        capture=True,
    )
    log = r.stdout
    if "Bootstrapped admin user" in log:
        return "created"
    if "User bootstrap skipped" in log or "bootstrap skipped" in log.lower():
        return "skipped"
    return None


def deploy_to_target(target: DeploymentTarget, artifacts: BuildArtifacts) -> DeployResult:
    ensure_tool("ssh")
    ensure_tool("scp")
    ensure_remote_layout(target)
    uploaded = upload_release(target, artifacts)
    config_created, config_updated, gen_user, gen_pass = ensure_remote_config(target)
    unit_updated = ensure_systemd_unit(target)
    activated = activate_release(target, artifacts)
    enabled, restarted = ensure_service(
        target,
        unit_updated=unit_updated,
        release_activated=activated,
        config_changed=(config_created or config_updated),
    )
    healthy = healthcheck(target)

    # When credentials were generated, verify the server actually bootstrapped them.
    bootstrap_status: str | None = None
    if gen_user or gen_pass:
        time.sleep(2)  # brief settle time for journal flush
        svc = detect_service_name(target)
        bootstrap_status = _check_bootstrap_journal(target, svc)
        if bootstrap_status == "skipped":
            console.print(Panel(
                "[bold yellow]⚠  Bootstrap was skipped[/bold yellow]\n\n"
                "The server already has existing users in its database.\n"
                "The credentials shown above were written to config.toml but "
                "[bold]were NOT applied[/bold] — the database was not empty.\n\n"
                "To fix this, do one of:\n"
                "  • Log in with your existing account\n"
                "  • SSH in and delete [cyan]shared/codex.db[/cyan] to reset, then restart the service\n"
                f"  • Run [cyan]codex.py configure --target <name> --reset-admin[/cyan]",
                title="[bold red]Credentials may not work[/bold red]",
                border_style="red",
            ))
        elif bootstrap_status == "created":
            ok("Bootstrap confirmed — admin user created from generated credentials")

    return DeployResult(
        release_uploaded=uploaded,
        release_activated=activated,
        config_created=config_created,
        config_updated=config_updated,
        unit_updated=unit_updated,
        service_restarted=restarted,
        service_enabled=enabled,
        healthcheck_ok=healthy,
        generated_admin_username=gen_user,
        generated_admin_password=gen_pass,
        bootstrap_status=bootstrap_status,
    )


# ---------------------------------------------------------------------------
# Configure helpers
# ---------------------------------------------------------------------------


def _generate_credentials() -> tuple[str, str]:
    username = f"admin-{secrets.token_hex(4)}"
    alphabet = string.ascii_letters + string.digits + "-_."
    password = "".join(secrets.choice(alphabet) for _ in range(28))
    return username, password


def _render_config(
    target: DeploymentTarget,
    *,
    bootstrap_user: str | None = None,
    bootstrap_pass: str | None = None,
) -> str:
    lines = CONFIG_TEMPLATE.read_text(encoding="utf-8").splitlines()
    rendered: list[str] = []
    section = ""
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            section = stripped.strip("[]")
            rendered.append(line)
            continue
        if section == "server" and stripped.startswith("host ="):
            rendered.append('host = "0.0.0.0"')
        elif section == "server" and stripped.startswith("port ="):
            rendered.append(f"port = {target.http_port}")
        elif section == "database" and stripped.startswith("path ="):
            rendered.append('path = "./codex.db"')
        elif section == "auth" and stripped.startswith("enabled ="):
            rendered.append("enabled = true")
        elif section == "auth" and stripped.startswith("bootstrap_admin_username =") and bootstrap_user:
            rendered.append(f'bootstrap_admin_username = "{bootstrap_user}"')
        elif section == "auth" and stripped.startswith("bootstrap_admin_password =") and bootstrap_pass:
            rendered.append(f'bootstrap_admin_password = "{bootstrap_pass}"')
        else:
            rendered.append(line)
    return "\n".join(rendered).strip() + "\n"


def _patch_bootstrap_creds(config_text: str) -> tuple[str, bool, str | None, str | None]:
    lines = config_text.splitlines()
    section = ""
    u_idx = p_idx = enabled_idx = None
    existing_user = existing_pass = None
    u_pat = re.compile(r'^\s*bootstrap_admin_username\s*=\s*"(?P<v>[^"]*)"\s*')
    p_pat = re.compile(r'^\s*bootstrap_admin_password\s*=\s*"(?P<v>[^"]*)"\s*')
    e_pat = re.compile(r'^\s*enabled\s*=\s*(true|false)')
    for i, line in enumerate(lines):
        s = line.strip()
        if s.startswith("[") and s.endswith("]"):
            section = s.strip("[]")
            continue
        if section != "auth":
            continue
        m = u_pat.match(line)
        if m:
            u_idx, existing_user = i, m.group("v")
            continue
        m = p_pat.match(line)
        if m:
            p_idx, existing_pass = i, m.group("v")
            continue
        if e_pat.match(line):
            enabled_idx = i
    if u_idx is None or p_idx is None:
        return config_text, False, None, None
    if existing_user and existing_pass:
        return config_text, False, None, None
    gen_user, gen_pass = _generate_credentials()
    final_user = existing_user or gen_user
    final_pass = existing_pass or gen_pass
    lines[u_idx] = f'bootstrap_admin_username = "{final_user}"'
    lines[p_idx] = f'bootstrap_admin_password = "{final_pass}"'
    if enabled_idx is not None:
        lines[enabled_idx] = "enabled = true"
    updated = "\n".join(lines)
    if config_text.endswith("\n"):
        updated += "\n"
    return updated, True, gen_user if not existing_user else None, gen_pass if not existing_pass else None


def _upload_config(target: DeploymentTarget, text: str) -> None:
    with tempfile.NamedTemporaryFile("w", suffix=".toml", delete=False) as f:
        f.write(text)
        tmp = Path(f.name)
    try:
        remote_tmp = f"{target.tmp_dir}/config.toml"
        scp_upload(target, tmp, remote_tmp)
        ssh_cmd(target, " && ".join([
            f"mv {shell_quote(remote_tmp)} {shell_quote(target.remote_config_path)}",
            f"chmod 640 {shell_quote(target.remote_config_path)}",
        ]))
    finally:
        tmp.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# Preflight / doctor
# ---------------------------------------------------------------------------


def run_preflight(target: DeploymentTarget, *, remote_build: bool = False) -> list[PreflightCheck]:
    checks: list[PreflightCheck] = []

    def chk(name: str, ok_: bool, detail: str) -> None:
        checks.append(PreflightCheck(name, ok_, detail))

    def tool_chk(tool: str) -> None:
        p = shutil.which(tool)
        chk(f"local:{tool}", p is not None, p or "not found on PATH")

    tool_chk("ssh")
    tool_chk("scp")
    if not remote_build:
        tool_chk("npm")
        tool_chk("cargo")
        if platform.system().lower() != "linux":
            tool_chk("cross")
            tool_chk("docker")
            if shutil.which("docker"):
                try:
                    run_cmd(["docker", "version", "-f", "{{.Server.Os}}"], cwd=REPO_ROOT, capture=True)
                    chk("local:docker-daemon", True, "reachable")
                except CodexError as e:
                    chk("local:docker-daemon", False, str(e))

    try:
        r = ssh_cmd(target, "echo preflight-ok", capture=True)
        ok_ = r.stdout.strip() == "preflight-ok"
        chk("remote:ssh", ok_, "connected" if ok_ else "unexpected response")
    except CodexError as e:
        chk("remote:ssh", False, str(e))
        return checks

    for name, probe in [
        ("bash", "command -v bash"),
        ("tar", "command -v tar"),
        ("systemctl", "command -v systemctl"),
        ("cargo", "command -v cargo"),
        ("npm", "command -v npm"),
    ]:
        try:
            ssh_cmd(target, probe, capture=True)
            chk(f"remote:{name}", True, "available")
        except CodexError as e:
            chk(f"remote:{name}", False, str(e))

    try:
        ssh_cmd(target, "sudo -n true", capture=True)
        chk("remote:sudo-nopasswd", True, "enabled")
    except CodexError as e:
        chk("remote:sudo-nopasswd", False, str(e))

    return checks


# ---------------------------------------------------------------------------
# Docker check
# ---------------------------------------------------------------------------


def _ensure_docker() -> None:
    ensure_tool("docker")
    try:
        run_cmd(["docker", "version", "-f", "{{.Server.Os}}"], cwd=REPO_ROOT, capture=True)
    except CodexError as e:
        raise CodexError(f"Docker daemon not reachable. Start Docker and retry. ({e})") from e


# ---------------------------------------------------------------------------
# Source archive (for remote build)
# ---------------------------------------------------------------------------


def _create_source_archive(archive_path: Path) -> None:
    exclude_roots = {".git", "target", "dist", "logs", "test-results"}
    exclude_sub = {"frontend/node_modules", "frontend/test-results", "frontend/logs"}
    with tarfile.open(archive_path, "w:gz") as arc:
        for path in REPO_ROOT.rglob("*"):
            rel = path.relative_to(REPO_ROOT)
            rel_posix = rel.as_posix()
            if not rel_posix:
                continue
            if rel.parts[0] in exclude_roots:
                continue
            if any(rel_posix == s or rel_posix.startswith(f"{s}/") for s in exclude_sub):
                continue
            arc.add(path, arcname=rel_posix)


# ---------------------------------------------------------------------------
# Click CLI
# ---------------------------------------------------------------------------


# ---------------------------------------------------------------------------
# Local install helpers
# ---------------------------------------------------------------------------

def _default_install_dir() -> Path:
    """User-writable bin dir that is typically on $PATH."""
    xdg_home = os.environ.get("XDG_DATA_HOME")
    if xdg_home:
        p = Path(xdg_home).parent / "bin"
    else:
        p = Path.home() / ".local" / "bin"
    return p


def _default_config_dir() -> Path:
    xdg = os.environ.get("XDG_CONFIG_HOME")
    return Path(xdg) / "codex" if xdg else Path.home() / ".config" / "codex"


def local_install(
    *,
    build_first: bool,
    release: bool,
    install_dir: Path,
    config_dir: Path,
    with_desktop: bool,
    admin_password: str | None,
    port: int,
) -> None:
    """Build (optionally) then copy binaries and config to local install locations."""
    dummy_target = DeploymentTarget(
        name="local",
        ssh_host="",
        ssh_user="",
        ssh_port=22,
        ip_address="127.0.0.1",
        http_port=port,
        app_dir=str(install_dir),
    )

    if build_first:
        artifacts = assemble_dist(
            dummy_target,
            build_desktop_bin=with_desktop,
            release=release,
            skip_support=True,
        )
    else:
        artifacts = load_existing_artifacts(dummy_target)

    # ── install binary ────────────────────────────────────────────────────
    step("Installing server binary")
    install_dir.mkdir(parents=True, exist_ok=True)
    dest_bin = install_dir / BIN_NAME
    shutil.copy2(artifacts.server_binary, dest_bin)
    dest_bin.chmod(dest_bin.stat().st_mode | 0o111)
    ok(f"Server binary → {dest_bin}")

    # ── install desktop binary (optional) ────────────────────────────────
    desktop_src = DIST_DIR / "desktop" / DESKTOP_BIN_NAME
    if with_desktop and desktop_src.exists():
        step("Installing desktop binary")
        dest_desktop = install_dir / DESKTOP_BIN_NAME
        shutil.copy2(desktop_src, dest_desktop)
        dest_desktop.chmod(dest_desktop.stat().st_mode | 0o111)
        ok(f"Desktop binary → {dest_desktop}")
    elif with_desktop:
        warn(f"Desktop binary not found at {desktop_src} — skipping")

    # ── install / create config ───────────────────────────────────────────
    step("Installing config")
    config_dir.mkdir(parents=True, exist_ok=True)
    dest_cfg = config_dir / "config.toml"

    if dest_cfg.exists():
        info(f"Config already exists at {dest_cfg} — preserving (not overwritten)")
    else:
        lines = CONFIG_TEMPLATE.read_text(encoding="utf-8").splitlines()
        rendered: list[str] = []
        section = ""
        for line in lines:
            stripped = line.strip()
            if stripped.startswith("[") and stripped.endswith("]"):
                section = stripped.strip("[]")
                rendered.append(line)
                continue
            if section == "server" and stripped.startswith("host ="):
                rendered.append('host = "127.0.0.1"')
            elif section == "server" and stripped.startswith("port ="):
                rendered.append(f"port = {port}")
            elif section == "database" and stripped.startswith("path ="):
                rendered.append(f'path = "{config_dir / "codex.db"}"')
            elif section == "auth" and stripped.startswith("enabled ="):
                rendered.append("enabled = true")
            elif section == "auth" and stripped.startswith("bootstrap_admin_username ="):
                rendered.append('bootstrap_admin_username = "admin"')
            elif section == "auth" and stripped.startswith("bootstrap_admin_password =") and admin_password:
                rendered.append(f'bootstrap_admin_password = "{admin_password}"')
            else:
                rendered.append(line)
        dest_cfg.write_text("\n".join(rendered).strip() + "\n", encoding="utf-8")
        ok(f"Config created → {dest_cfg}")
        if admin_password:
            console.print(Panel(
                f"[bold]username:[/bold] admin\n[bold]password:[/bold] {admin_password}",
                title="[yellow]Bootstrap admin credentials[/yellow]",
                border_style="yellow",
            ))
        else:
            warn("No --admin-password set. Edit the config and set bootstrap_admin_password before first run.")

    # ── summary ───────────────────────────────────────────────────────────
    console.print(Panel(
        f"[bold]Binary:[/bold]  {dest_bin}\n"
        f"[bold]Config:[/bold]  {dest_cfg}\n\n"
        f"Run with:\n  [cyan]{dest_bin} --config {dest_cfg}[/cyan]\n\n"
        f"Then open [cyan]http://127.0.0.1:{port}[/cyan]",
        title="[bold green]Local install complete[/bold green]",
        border_style="green",
    ))

    if install_dir not in [Path(p) for p in os.environ.get("PATH", "").split(os.pathsep)]:
        warn(f"{install_dir} is not on your PATH. Add it with:\n  export PATH=\"{install_dir}:$PATH\"")


@click.group(context_settings={"help_option_names": ["-h", "--help"]})
def cli() -> None:
    """Codex — build, deploy, configure, and test the Codex server and desktop."""


# ── build ────────────────────────────────────────────────────────────────────

@cli.command()
@click.option("--server", "component", flag_value="server", help="Build server only")
@click.option("--desktop", "component", flag_value="desktop", help="Build desktop only")
@click.option("--all", "component", flag_value="all", default=True, help="Build server + desktop (default)")
@click.option("--frontend/--no-frontend", default=True, show_default=True, help="Also build frontend before server")
@click.option("--release/--debug", default=True, show_default=True, help="Cargo profile")
@click.option("--cross", "cross_target", metavar="TRIPLE", default=None, help=f"Cross-compile to target triple (e.g. {LINUX_TARGET})")
@click.option("--remote-build", is_flag=True, help="Build on the remote VM instead of locally")
@click.option("--target", "target_name", default=None, metavar="NAME", help="Target from targets.toml (for --remote-build)")
@click.option("--skip-support", is_flag=True, help="Skip building codex-client / codex-types")
@click.pass_context
def build(
    ctx: click.Context,
    component: str,
    frontend: bool,
    release: bool,
    cross_target: str | None,
    remote_build: bool,
    target_name: str | None,
    skip_support: bool,
) -> None:
    """Build server and/or desktop binaries."""
    try:
        if remote_build:
            targets = load_targets()
            target = pick_target(target_name, targets)
            step("Remote build requested")
            if component in ("server", "all"):
                build_server_remote(target)
            if component in ("desktop", "all"):
                warn("Remote desktop build not supported — desktop requires a local display. Build locally.")
            return

        # Standalone build: no deploy target, so use a dummy name for the manifest.
        dummy_target = DeploymentTarget(
            name="local",
            ssh_host="",
            ssh_user="",
            ssh_port=22,
            ip_address="",
            http_port=8080,
            app_dir="/opt/codex",
        )

        if component == "server":
            if frontend:
                build_frontend()
            build_server(release=release, cross_target=cross_target)
            if not skip_support:
                step("Building supporting crates")
                run_cmd(["cargo", "build", "-p", "codex-client"], cwd=REPO_ROOT)
                run_cmd(["cargo", "build", "-p", "codex-types"], cwd=REPO_ROOT)
            assemble_dist(dummy_target, build_desktop_bin=False, release=release, cross_target=cross_target, skip_support=True)
        elif component == "desktop":
            build_desktop(release=release, cross_target=cross_target)
        else:  # "all"
            assemble_dist(
                dummy_target,
                build_desktop_bin=True,
                release=release,
                cross_target=cross_target,
                skip_support=skip_support,
            )

        console.print(Panel("[bold green]Build complete[/bold green]", border_style="green"))
    except CodexError as e:
        fail(str(e))
        raise SystemExit(1)


# ── local-install ────────────────────────────────────────────────────────────

@cli.command("local-install")
@click.option("--build-first/--no-build", default=True, show_default=True,
              help="Build before installing (default: yes)")
@click.option("--release/--debug", default=True, show_default=True, help="Cargo profile")
@click.option("--install-dir", type=click.Path(path_type=Path), default=None, metavar="DIR",
              help=f"Directory to install binaries into (default: ~/.local/bin)")
@click.option("--config-dir", type=click.Path(path_type=Path), default=None, metavar="DIR",
              help="Directory to install config.toml into (default: ~/.config/codex)")
@click.option("--with-desktop/--no-desktop", default=False, show_default=True,
              help="Also build and install the desktop client binary")
@click.option("--admin-password", default=None, metavar="PASS", envvar="CODEX_ADMIN_PASSWORD",
              help="Bootstrap admin password written to config (env: CODEX_ADMIN_PASSWORD)")
@click.option("--port", default=8080, show_default=True, metavar="PORT",
              help="Port to set in the generated config")
def local_install_cmd(
    build_first: bool,
    release: bool,
    install_dir: Path | None,
    config_dir: Path | None,
    with_desktop: bool,
    admin_password: str | None,
    port: int,
) -> None:
    """Build and install Codex locally.

    Copies the server binary to INSTALL_DIR (default: ~/.local/bin) and writes
    a starter config.toml to CONFIG_DIR (default: ~/.config/codex).
    An existing config is never overwritten.

    Examples:

    \b
      # Build + install to ~/.local/bin, prompt for nothing:
      python scripts/codex.py local-install --admin-password mysecret

    \b
      # Install from an already-built dist/ without rebuilding:
      python scripts/codex.py local-install --no-build

    \b
      # Install to /usr/local/bin (may require sudo for the copy):
      python scripts/codex.py local-install --install-dir /usr/local/bin
    """
    effective_install_dir = install_dir or _default_install_dir()
    effective_config_dir = config_dir or _default_config_dir()
    try:
        local_install(
            build_first=build_first,
            release=release,
            install_dir=effective_install_dir,
            config_dir=effective_config_dir,
            with_desktop=with_desktop,
            admin_password=admin_password,
            port=port,
        )
    except CodexError as e:
        fail(str(e))
        raise SystemExit(1)


# ── deploy ───────────────────────────────────────────────────────────────────

@cli.command()
@click.argument("target_name", metavar="TARGET", required=False)
@click.option("--build-first", is_flag=True, help="Build before deploying")
@click.option("--remote-build", is_flag=True, help="With --build-first: build on the remote VM")
@click.option("--skip-support", is_flag=True, help="Skip supporting crate builds")
@click.option("--targets-file", type=click.Path(path_type=Path), default=TARGETS_FILE, show_default=True)
def deploy(
    target_name: str | None,
    build_first: bool,
    remote_build: bool,
    skip_support: bool,
    targets_file: Path,
) -> None:
    """Deploy Codex server to a remote target.

    TARGET is a name from targets.toml. Prompted interactively if omitted.
    """
    try:
        targets = load_targets(targets_file)
        target = pick_target(target_name, targets)

        if build_first:
            artifacts = assemble_dist(target, remote_build=remote_build, skip_support=skip_support)
        else:
            artifacts = load_existing_artifacts(target)

        result = deploy_to_target(target, artifacts)

        table = Table(title="Deploy summary", box=box.SIMPLE_HEAVY)
        table.add_column("Step")
        table.add_column("Result")
        rows = [
            ("Release uploaded", result.release_uploaded),
            ("Release activated", result.release_activated),
            ("Config created", result.config_created),
            ("Config updated", result.config_updated),
            ("Systemd unit updated", result.unit_updated),
            ("Service restarted", result.service_restarted),
            ("Service enabled", result.service_enabled),
            ("Health check", result.healthcheck_ok),
        ]
        for label, value in rows:
            table.add_row(label, "[green]yes[/green]" if value else "[dim]no[/dim]")
        console.print(table)

        if not result.healthcheck_ok:
            raise SystemExit(1)
    except CodexError as e:
        fail(str(e))
        raise SystemExit(1)


# ── migrate ──────────────────────────────────────────────────────────────────

_LEGACY_SERVICE = "obsidian-host"
_LEGACY_APP_DIRS = ["/opt/obsidian-host"]


@cli.command()
@click.argument("target_name", metavar="TARGET", required=False)
@click.option("--targets-file", type=click.Path(path_type=Path), default=TARGETS_FILE, show_default=True)
@click.option("--remove-legacy-dir", is_flag=True, help=f"Also delete the old app directory ({_LEGACY_APP_DIRS})")
@click.option("--yes", is_flag=True, help="Skip confirmation prompts")
def migrate(
    target_name: str | None,
    targets_file: Path,
    remove_legacy_dir: bool,
    yes: bool,
) -> None:
    """Remove legacy obsidian-host service and redeploy as codex.

    Stops and removes the old 'obsidian-host' systemd unit, then runs a full
    deploy so the server runs under the 'codex' service name.
    """
    try:
        targets = load_targets(targets_file)
        target = pick_target(target_name, targets)

        if not yes:
            console.print(
                Panel(
                    f"This will:\n"
                    f"  1. Stop and disable [yellow]{_LEGACY_SERVICE}[/yellow] on [cyan]{target.ssh_host}[/cyan]\n"
                    f"  2. Remove [yellow]/etc/systemd/system/{_LEGACY_SERVICE}.service[/yellow]\n"
                    f"  3. Run a fresh deploy to install the [green]{SERVICE_NAME}[/green] service\n"
                    + (f"  4. Delete legacy app dirs: {_LEGACY_APP_DIRS}\n" if remove_legacy_dir else ""),
                    title="[bold]Migration plan[/bold]",
                    border_style="yellow",
                )
            )
            click.confirm("Proceed?", abort=True)

        # ── 1. Stop + disable legacy service ─────────────────────────────
        step(f"Stopping legacy service '{_LEGACY_SERVICE}'")
        r = ssh_cmd(
            target,
            f"systemctl is-active {shell_quote(_LEGACY_SERVICE)} >/dev/null 2>&1 && echo active || echo inactive",
            capture=True,
        )
        if r.stdout.strip() == "active":
            ssh_cmd(target, f"sudo systemctl stop {shell_quote(_LEGACY_SERVICE)}")
            ok(f"Stopped {_LEGACY_SERVICE}")
        else:
            info(f"{_LEGACY_SERVICE} is not active — skipping stop")

        ssh_cmd(
            target,
            f"sudo systemctl disable {shell_quote(_LEGACY_SERVICE)} 2>/dev/null || true",
        )

        # ── 2. Remove legacy unit file ────────────────────────────────────
        legacy_unit = f"/etc/systemd/system/{_LEGACY_SERVICE}.service"
        step(f"Removing legacy unit file {legacy_unit}")
        r2 = ssh_cmd(
            target,
            f"[ -f {shell_quote(legacy_unit)} ] && echo yes || echo no",
            capture=True,
        )
        if r2.stdout.strip() == "yes":
            ssh_cmd(target, f"sudo rm -f {shell_quote(legacy_unit)}")
            ok("Legacy unit file removed")
        else:
            info("Legacy unit file not found — already clean")

        ssh_cmd(target, "sudo systemctl daemon-reload && sudo systemctl reset-failed 2>/dev/null || true")
        ok("systemd reloaded")

        # ── 3. Remove legacy app directories (optional) ───────────────────
        if remove_legacy_dir:
            step("Removing legacy app directories")
            for d in _LEGACY_APP_DIRS:
                r3 = ssh_cmd(target, f"[ -d {shell_quote(d)} ] && echo yes || echo no", capture=True)
                if r3.stdout.strip() == "yes":
                    ssh_cmd(target, f"sudo rm -rf {shell_quote(d)}")
                    ok(f"Removed {d}")
                else:
                    info(f"{d} not found — skipping")

        # ── 4. Deploy codex ───────────────────────────────────────────────
        step("Deploying codex service")
        artifacts = load_existing_artifacts(target)
        result = deploy_to_target(target, artifacts)

        table = Table(title="Migration complete", box=box.SIMPLE_HEAVY)
        table.add_column("Step")
        table.add_column("Result")
        rows = [
            ("Release uploaded", result.release_uploaded),
            ("Release activated", result.release_activated),
            ("Config created", result.config_created),
            ("Config updated", result.config_updated),
            ("Systemd unit updated", result.unit_updated),
            ("Service restarted", result.service_restarted),
            ("Health check", result.healthcheck_ok),
        ]
        for label, value in rows:
            table.add_row(label, "[green]yes[/green]" if value else "[dim]no[/dim]")
        console.print(table)

        if not result.healthcheck_ok:
            raise SystemExit(1)

    except CodexError as e:
        fail(str(e))
        raise SystemExit(1)


def _reset_remote_admin(target: DeploymentTarget) -> None:
    """Back up and remove the remote DB so the server recreates it empty, then re-bootstrap."""
    db_path = f"{target.shared_dir}/codex.db"
    db_backup = f"{target.shared_dir}/codex.db.bak"

    warn("This will back up and remove the remote database (all data lost) and restart the service.")
    click.confirm("Continue?", abort=True)

    step("Detecting service name")
    svc = detect_service_name(target)
    info(f"Using service: {svc!r}")

    step("Stopping service")
    ssh_cmd(target, f"sudo systemctl stop {shell_quote(svc)}")
    ok(f"Service {svc!r} stopped")

    step("Backing up and removing remote DB")
    if _remote_path_exists(target, db_path):
        ssh_cmd(target, f"mv {shell_quote(db_path)} {shell_quote(db_backup)}")
        ok(f"Database moved to {db_backup}")
    else:
        info("No existing database found — will be created fresh on start")

    step("Generating new admin credentials")
    username, password = _generate_credentials()
    updated = _render_config(target, bootstrap_user=username, bootstrap_pass=password)
    _upload_config(target, updated)
    ok("Config updated with new credentials")

    step("Starting service")
    ssh_cmd(target, f"sudo systemctl start {shell_quote(svc)}")
    ok(f"Service {svc!r} started")

    time.sleep(3)
    bootstrap_status = _check_bootstrap_journal(target, svc)
    if bootstrap_status == "created":
        ok("Bootstrap confirmed")
    elif bootstrap_status == "skipped":
        warn("Bootstrap still shows as skipped — check service logs")
    else:
        info("Bootstrap status unknown — check logs with: codex.py logs --target <name>")

    console.print(Panel(
        f"[bold]username:[/bold] {username}\n[bold]password:[/bold] {password}",
        title="[green]New admin credentials[/green]",
        border_style="green",
    ))


# ── configure ────────────────────────────────────────────────────────────────

@cli.command()
@click.option("--target", "target_name", default=None, metavar="NAME", help="Operate on a remote target's config")
@click.option("--show", is_flag=True, help="Print the current config")
@click.option("--init", is_flag=True, help="Push a rendered config to the remote target")
@click.option("--bootstrap-admin", is_flag=True, help="Ensure bootstrap admin credentials are set")
@click.option("--reset-admin", is_flag=True, help="Delete all users from the remote DB, set new credentials, and restart service")
@click.option("--targets-file", type=click.Path(path_type=Path), default=TARGETS_FILE, show_default=True)
def configure(
    target_name: str | None,
    show: bool,
    init: bool,
    bootstrap_admin: bool,
    reset_admin: bool,
    targets_file: Path,
) -> None:
    """Manage local or remote Codex configuration."""
    try:
        if target_name:
            targets = load_targets(targets_file)
            target = pick_target(target_name, targets)

            if show:
                step(f"Remote config: {target.remote_config_path}")
                r = ssh_cmd(target, f"cat {shell_quote(target.remote_config_path)}", capture=True)
                console.print(r.stdout)
                return

            if init:
                username, password = _generate_credentials()
                config_text = _render_config(target, bootstrap_user=username, bootstrap_pass=password)
                _upload_config(target, config_text)
                ok(f"Config initialised at {target.remote_config_path}")
                console.print(Panel(f"[bold]username:[/bold] {username}\n[bold]password:[/bold] {password}", title="[yellow]Admin credentials[/yellow]", border_style="yellow"))
                return

            if bootstrap_admin:
                r = ssh_cmd(target, f"cat {shell_quote(target.remote_config_path)}", capture=True)
                updated, changed, gen_user, gen_pass = _patch_bootstrap_creds(r.stdout)
                if not changed:
                    ok("Bootstrap credentials already set — nothing to do")
                else:
                    _upload_config(target, updated)
                    ok("Bootstrap credentials updated")
                    if gen_user:
                        console.print(Panel(f"[bold]username:[/bold] {gen_user}\n[bold]password:[/bold] {gen_pass}", title="[yellow]Admin credentials[/yellow]", border_style="yellow"))
                return

            if reset_admin:
                _reset_remote_admin(target)
                return

            console.print("Specify --show, --init, --bootstrap-admin, or --reset-admin")
        else:
            if reset_admin or bootstrap_admin or init:
                fail("--target NAME is required for this operation")
                raise SystemExit(1)
            if show:
                console.print(CONFIG_TEMPLATE.read_text(encoding="utf-8"))
            else:
                console.print(Panel(f"Local config: [cyan]{CONFIG_TEMPLATE}[/cyan]\nEdit it directly or use --target NAME for remote config.", border_style="dim"))
    except CodexError as e:
        fail(str(e))
        raise SystemExit(1)


# ── test ─────────────────────────────────────────────────────────────────────

@cli.command()
@click.option("--unit", "suite", flag_value="unit", help="Rust unit tests only")
@click.option("--integration", "suite", flag_value="integration", help="Rust integration tests only")
@click.option("--e2e", "suite", flag_value="e2e", help="Playwright frontend E2E tests")
@click.option("--desktop", "suite", flag_value="desktop", help="Desktop headless tests")
@click.option("--all", "suite", flag_value="all", default=True, help="Run all suites (default)")
@click.option("--filter", "test_filter", default=None, metavar="PATTERN", help="Filter test names (passed to cargo test)")
@click.option("--no-fail-fast", is_flag=True, help="Continue running suites after a failure")
def test(
    suite: str,
    test_filter: str | None,
    no_fail_fast: bool,
) -> None:
    """Run test suites (unit, integration, e2e, desktop)."""
    passed: list[str] = []
    failed: list[str] = []

    def run_suite(name: str, fn) -> None:  # type: ignore[no-untyped-def]
        step(name)
        try:
            fn()
            passed.append(name)
            ok(f"{name} passed")
        except CodexError as e:
            failed.append(name)
            fail(f"{name} failed: {e}")
            if not no_fail_fast:
                _print_test_summary(passed, failed)
                raise SystemExit(1)

    filter_args = [test_filter] if test_filter else []

    if suite in ("unit", "all"):
        def _unit() -> None:
            run_cmd(["cargo", "test", "--lib", "--bins", *filter_args], cwd=REPO_ROOT)
        run_suite("Unit tests", _unit)

    if suite in ("integration", "all"):
        def _integration() -> None:
            run_cmd(["cargo", "test", "--tests", *filter_args], cwd=REPO_ROOT)
        run_suite("Integration tests", _integration)

    if suite in ("desktop", "all"):
        def _desktop() -> None:
            run_cmd(
                ["cargo", "test", "-p", "codex-desktop", "--tests", *filter_args],
                cwd=REPO_ROOT,
            )
        run_suite("Desktop tests", _desktop)

    if suite in ("e2e", "all"):
        if not FRONTEND_DIR.exists():
            warn("Frontend directory not found — skipping e2e")
        else:
            def _e2e() -> None:
                if not FRONTEND_NODE_MODULES.exists():
                    run_cmd(["npm", "ci"], cwd=FRONTEND_DIR)
                run_cmd(["npx", "playwright", "test"], cwd=FRONTEND_DIR)
            run_suite("Playwright e2e", _e2e)

    _print_test_summary(passed, failed)
    if failed:
        raise SystemExit(1)


def _print_test_summary(passed: list[str], failed: list[str]) -> None:
    table = Table(title="Test results", box=box.SIMPLE)
    table.add_column("Suite")
    table.add_column("Result")
    for s in passed:
        table.add_row(s, "[bold green]PASS[/bold green]")
    for s in failed:
        table.add_row(s, "[bold red]FAIL[/bold red]")
    console.print(table)


# ── doctor ───────────────────────────────────────────────────────────────────

@cli.command()
@click.argument("target_name", metavar="TARGET", required=False)
@click.option("--remote-build", is_flag=True, help="Check prerequisites for remote-build strategy")
@click.option("--targets-file", type=click.Path(path_type=Path), default=TARGETS_FILE, show_default=True)
def doctor(
    target_name: str | None,
    remote_build: bool,
    targets_file: Path,
) -> None:
    """Run preflight checks for a deployment target."""
    try:
        targets = load_targets(targets_file)
        target = pick_target(target_name, targets)
        checks = run_preflight(target, remote_build=remote_build)

        table = Table(title=f"Preflight: {target.name}", box=box.SIMPLE_HEAVY)
        table.add_column("Check")
        table.add_column("Status", width=8)
        table.add_column("Detail")
        for c in checks:
            status = "[bold green]PASS[/bold green]" if c.ok else "[bold red]FAIL[/bold red]"
            table.add_row(c.name, status, c.detail)
        console.print(table)

        if any(not c.ok for c in checks):
            fail("Preflight failed — resolve issues above before deploying")
            raise SystemExit(1)
        ok("All preflight checks passed")
    except CodexError as e:
        fail(str(e))
        raise SystemExit(1)


# ── targets ──────────────────────────────────────────────────────────────────

@cli.command()
@click.option("--targets-file", type=click.Path(path_type=Path), default=TARGETS_FILE, show_default=True)
def targets(targets_file: Path) -> None:
    """List deployment targets from targets.toml."""
    try:
        all_targets = load_targets(targets_file)
        table = Table(title="Deployment targets", box=box.SIMPLE_HEAVY)
        table.add_column("Name", style="bold cyan")
        table.add_column("SSH")
        table.add_column("URL")
        table.add_column("App dir")
        for t in all_targets.values():
            table.add_row(
                t.name,
                f"{t.ssh_user}@{t.ssh_host}:{t.ssh_port}",
                f"http://{t.ip_address}:{t.http_port}",
                t.app_dir,
            )
        console.print(table)
    except CodexError as e:
        fail(str(e))
        raise SystemExit(1)


# ── status ───────────────────────────────────────────────────────────────────

@cli.command()
@click.argument("target_name", metavar="TARGET", required=False)
@click.option("--token", envvar="CODEX_TOKEN", default=None, metavar="JWT", help="Bearer token for authenticated requests (or set CODEX_TOKEN)")
@click.option("--targets-file", type=click.Path(path_type=Path), default=TARGETS_FILE, show_default=True)
def status(
    target_name: str | None,
    token: str | None,
    targets_file: Path,
) -> None:
    """Check whether a target is running and show its version."""
    try:
        all_targets = load_targets(targets_file)
        target = pick_target(target_name, all_targets)
        base_url = f"http://{target.ip_address}:{target.http_port}"

        # ── systemd service state (via SSH) ──────────────────────────────
        service_active = service_sub = service_pid = service_since = "—"
        service_ok = False
        try:
            service_name_found = detect_service_name(target)

            r3 = ssh_cmd(
                target,
                f"systemctl show {shell_quote(service_name_found)} "
                "--property=ActiveState,SubState,MainPID,ExecMainStartTimestamp --no-pager",
                capture=True,
            )
            service_props: dict[str, str] = {}
            for line in r3.stdout.splitlines():
                if "=" in line:
                    k, _, v = line.partition("=")
                    service_props[k.strip()] = v.strip()
            service_active = service_props.get("ActiveState", "unknown")
            service_sub = service_props.get("SubState", "unknown")
            service_pid = service_props.get("MainPID", "unknown")
            service_since = service_props.get("ExecMainStartTimestamp", "unknown")
            service_ok = service_active == "active"
        except CodexError:
            service_active = "unreachable"
            service_name_found = SERVICE_NAME

        # ── HTTP helpers ──────────────────────────────────────────────────
        def http_get(path: str) -> tuple[int, dict]:
            """Return (status_code, body_dict). Never raises on HTTP errors."""
            req = urllib.request.Request(f"{base_url}{path}")
            if token:
                req.add_header("Authorization", f"Bearer {token}")
            try:
                with urllib.request.urlopen(req, timeout=5) as resp:
                    return resp.status, json.loads(resp.read().decode())
            except urllib.error.HTTPError as e:
                # Server responded — read body if possible
                try:
                    body = json.loads(e.read().decode())
                except Exception:
                    body = {}
                return e.code, body
            except (urllib.error.URLError, TimeoutError, OSError) as e:
                raise CodexError(f"Cannot reach {base_url}: {e}") from e

        # ── HTTP health endpoint ──────────────────────────────────────────
        health_status = "—"
        health_db = "—"
        health_ok = False
        health_note = ""
        try:
            code, data = http_get("/api/health")
            if code == 200:
                health_status = data.get("status", "unknown")
                health_db = data.get("database", "unknown")
                health_ok = health_status == "healthy"
            elif code == 401:
                health_status = "running"
                health_note = "(auth required — pass --token or set CODEX_TOKEN)"
                health_ok = True   # server is up, just protected
            else:
                health_status = f"HTTP {code}"
        except CodexError as e:
            health_status = f"unreachable — {e}"

        # ── HTTP version endpoint ─────────────────────────────────────────
        app_version = app_git = app_built = "—"
        version_note = ""
        try:
            code, data = http_get("/api/version")
            if code == 200:
                app_version = data.get("version", "unknown")
                app_git = data.get("git_hash", "unknown")
                app_built = data.get("build_date", "unknown")
            elif code == 401:
                version_note = "(auth required)"
        except CodexError:
            pass

        # ── Render ───────────────────────────────────────────────────────
        service_color = "green" if service_ok else "red"
        health_color = "green" if health_ok else "red"

        table = Table(box=box.SIMPLE_HEAVY, show_header=False, padding=(0, 2))
        table.add_column("Key", style="dim", width=22)
        table.add_column("Value")

        table.add_section()
        table.add_row("[bold]Target[/bold]", f"[cyan]{target.name}[/cyan]  ({target.ssh_user}@{target.ssh_host}:{target.ssh_port})")
        table.add_row("[bold]URL[/bold]", base_url)

        table.add_section()
        table.add_row("[bold]Service state[/bold]", f"[{service_color}]{service_active} ({service_sub})[/{service_color}]")
        service_name_note = f"  [dim](service: {service_name_found})[/dim]" if service_name_found != SERVICE_NAME else ""
        table.add_row("PID", service_pid + service_name_note)
        table.add_row("Running since", service_since)

        table.add_section()
        health_value = f"[{health_color}]{health_status}[/{health_color}]"
        if health_note:
            health_value += f"  [dim]{health_note}[/dim]"
        table.add_row("[bold]Health check[/bold]", health_value)
        table.add_row("Database", health_db)

        table.add_section()
        version_value = app_version
        if version_note:
            version_value += f"  [dim]{version_note}[/dim]"
        table.add_row("[bold]Version[/bold]", version_value)
        table.add_row("Git commit", app_git[:12] if app_git != "—" else "—")
        table.add_row("Build date", app_built)

        console.print(Panel(table, title=f"[bold]codex status — {target.name}[/bold]", border_style=service_color))

        if not service_ok or not health_ok:
            raise SystemExit(1)
    except CodexError as e:
        fail(str(e))
        raise SystemExit(1)


# ── logs ─────────────────────────────────────────────────────────────────────

@cli.command()
@click.argument("target_name", metavar="TARGET", required=False)
@click.option("--lines", "-n", default=50, show_default=True, help="Number of recent lines to show")
@click.option("--follow", "-f", is_flag=True, help="Follow the log stream (like journalctl -f)")
@click.option("--targets-file", type=click.Path(path_type=Path), default=TARGETS_FILE, show_default=True)
def logs(
    target_name: str | None,
    lines: int,
    follow: bool,
    targets_file: Path,
) -> None:
    """Stream systemd journal logs from a remote target."""
    try:
        all_targets = load_targets(targets_file)
        target = pick_target(target_name, all_targets)

        # Detect active service name via shared helper
        try:
            svc = detect_service_name(target)
        except CodexError:
            svc = SERVICE_NAME

        follow_flag = "-f" if follow else ""
        cmd = f"journalctl -u {shell_quote(svc)} -n {lines} {follow_flag} --no-pager"
        # Stream directly — don't capture, so output flows to the terminal in real time
        run_cmd(
            ["ssh", "-p", str(target.ssh_port), target.ssh_destination, cmd],
            cwd=REPO_ROOT,
            capture=False,
        )
    except CodexError as e:
        fail(str(e))
        raise SystemExit(1)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    cli()
