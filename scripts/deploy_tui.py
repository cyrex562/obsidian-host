#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
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
from typing import Iterable

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore


REPO_ROOT = Path(__file__).resolve().parents[1]
TARGETS_FILE = REPO_ROOT / "targets.toml"
CONFIG_TEMPLATE = REPO_ROOT / "config.toml"
SYSTEMD_TEMPLATE = REPO_ROOT / "deploy" / "systemd" / "obsidian-host.service.template"
DIST_DIR = REPO_ROOT / "dist"
FRONTEND_DIR = REPO_ROOT / "frontend"
FRONTEND_NODE_MODULES = FRONTEND_DIR / "node_modules"
FRONTEND_BUILD_OUTPUT = REPO_ROOT / "target" / "frontend"
LINUX_SERVER_TARGET = "x86_64-unknown-linux-gnu"
SERVICE_NAME = "obsidian-host"
BUILD_STRATEGY_LOCAL = "local"
BUILD_STRATEGY_REMOTE = "remote"


class DeployError(RuntimeError):
    pass


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
    target_platform: str
    server_binary: Path
    server_binary_name: str
    server_sha256: str
    dist_dir: Path
    manifest_path: Path
    tarball_path: Path
    config_template_path: Path
    desktop_placeholder_dir: Path
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


@dataclass
class PreflightCheck:
    name: str
    ok: bool
    detail: str


def info(message: str) -> None:
    print(f"[INFO] {message}")


def success(message: str) -> None:
    print(f"[ OK ] {message}")


def warn(message: str) -> None:
    print(f"[WARN] {message}")


def section(title: str) -> None:
    print(f"\n=== {title} ===")


def run_command(
    command: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    capture_output: bool = False,
) -> subprocess.CompletedProcess[str]:
    # On Windows, commands like 'npm' are batch files (.cmd) that subprocess
    # cannot find without shell=True. Resolve to the full path first.
    resolved_exe = shutil.which(command[0])
    if resolved_exe:
        command = [resolved_exe] + command[1:]
    info(f"Running: {' '.join(command)}")
    completed = subprocess.run(
        command,
        cwd=str(cwd) if cwd else None,
        env=env,
        text=True,
        check=False,
        capture_output=capture_output,
    )
    if completed.returncode != 0:
        stderr = completed.stderr.strip() if completed.stderr else ""
        stdout = completed.stdout.strip() if completed.stdout else ""
        details = stderr or stdout or f"Command exited with code {completed.returncode}"
        raise DeployError(details)
    return completed


def load_targets(targets_file: Path = TARGETS_FILE) -> dict[str, DeploymentTarget]:
    if not targets_file.exists():
        raise DeployError(f"Targets file not found: {targets_file}")

    data = tomllib.loads(targets_file.read_text(encoding="utf-8"))
    targets: dict[str, DeploymentTarget] = {}
    for name, raw_target in data.items():
        targets[name] = DeploymentTarget(
            name=name,
            ssh_host=raw_target["ssh_host"],
            ssh_user=raw_target["ssh_user"],
            ssh_port=int(raw_target.get("ssh_port", 22)),
            ip_address=raw_target["ip_address"],
            http_port=int(raw_target.get("http_port", 8080)),
            app_dir=raw_target["app_dir"],
        )
    return targets


def choose_target_interactively(
    targets: dict[str, DeploymentTarget],
) -> DeploymentTarget:
    if not targets:
        raise DeployError("No deployment targets found in targets.toml")

    names = list(targets)
    if len(names) == 1:
        return targets[names[0]]

    section("Available targets")
    for index, name in enumerate(names, start=1):
        target = targets[name]
        print(
            f"{index}. {name} ({target.ssh_user}@{target.ssh_host}:{target.ssh_port})"
        )

    while True:
        choice = input("Select target number: ").strip()
        if choice.isdigit() and 1 <= int(choice) <= len(names):
            return targets[names[int(choice) - 1]]
        warn("Please enter a valid target number.")


def get_git_commit() -> str:
    try:
        result = run_command(
            ["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, capture_output=True
        )
        return result.stdout.strip()
    except DeployError:
        return "unknown"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def ensure_tool_exists(tool_name: str) -> None:
    if shutil.which(tool_name) is None:
        raise DeployError(f"Required tool '{tool_name}' was not found on PATH")


def ensure_docker_daemon_ready() -> None:
    ensure_tool_exists("docker")
    try:
        run_command(
            [
                "docker",
                "version",
                "-f",
                "{{ .Server.Os }},,,{{ .Server.Arch }}",
            ],
            cwd=REPO_ROOT,
            capture_output=True,
        )
    except DeployError as error:
        raise DeployError(
            "Docker is installed but the daemon is not reachable. "
            "Start Docker Desktop (Linux engine) and retry local builds, "
            "or use --remote-build to compile on the target VM. "
            f"Details: {error}"
        ) from error


def build_frontend() -> None:
    ensure_tool_exists("npm")
    section("Building frontend")
    if not FRONTEND_NODE_MODULES.exists():
        run_command(["npm", "install"], cwd=FRONTEND_DIR)
    run_command(["npm", "run", "build"], cwd=FRONTEND_DIR)
    if not FRONTEND_BUILD_OUTPUT.exists():
        raise DeployError(
            f"Expected frontend build output missing: {FRONTEND_BUILD_OUTPUT}"
        )
    if not (FRONTEND_BUILD_OUTPUT / "index.html").exists():
        raise DeployError(
            "Frontend build output is incomplete: "
            f"{FRONTEND_BUILD_OUTPUT / 'index.html'} is missing"
        )
    success(f"Frontend built into {FRONTEND_BUILD_OUTPUT}")


def create_source_archive(archive_path: Path) -> None:
    exclude_roots = {
        ".git",
        "target",
        "dist",
        "logs",
        "test-results",
    }
    exclude_subpaths = {
        "frontend/node_modules",
        "frontend/test-results",
        "frontend/logs",
    }

    with tarfile.open(archive_path, "w:gz") as archive:
        for path in REPO_ROOT.rglob("*"):
            relative = path.relative_to(REPO_ROOT)
            relative_posix = relative.as_posix()

            if not relative_posix:
                continue
            if relative.parts[0] in exclude_roots:
                continue
            if any(
                relative_posix == subpath or relative_posix.startswith(f"{subpath}/")
                for subpath in exclude_subpaths
            ):
                continue

            archive.add(path, arcname=relative_posix)


def build_server_release(target_triple: str = LINUX_SERVER_TARGET) -> Path:
    section("Building embedded backend")

    # Force a rebuild of obsidian-server so freshly built frontend assets in
    # target/frontend are always re-embedded into the release binary.
    run_command(["cargo", "clean", "-p", "obsidian-server"], cwd=REPO_ROOT)

    if platform.system().lower() == "linux" and target_triple == LINUX_SERVER_TARGET:
        run_command(
            ["cargo", "build", "--release", "-p", "obsidian-server"], cwd=REPO_ROOT
        )
        binary = REPO_ROOT / "target" / "release" / "obsidian-host"
    else:
        ensure_tool_exists("cross")
        ensure_docker_daemon_ready()
        run_command(
            [
                "cross",
                "build",
                "--target",
                target_triple,
                "--release",
                "-p",
                "obsidian-server",
            ],
            cwd=REPO_ROOT,
        )
        binary = REPO_ROOT / "target" / target_triple / "release" / "obsidian-host"

    if not binary.exists():
        raise DeployError(f"Expected server binary not found: {binary}")
    success(f"Backend built: {binary}")
    return binary


def build_server_release_remote(target: DeploymentTarget) -> Path:
    ensure_tool_exists("ssh")
    ensure_tool_exists("scp")
    ensure_remote_layout(target)

    section("Building embedded backend (remote host)")
    build_id = datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S")
    local_archive = DIST_DIR / f"source-{build_id}.tar.gz"
    remote_build_root = f"{target.tmp_dir}/{SERVICE_NAME}-build-{build_id}"
    remote_archive = f"{remote_build_root}/source.tar.gz"
    remote_src_dir = f"{remote_build_root}/src"
    remote_binary = f"{remote_build_root}/obsidian-host"

    DIST_DIR.mkdir(parents=True, exist_ok=True)
    create_source_archive(local_archive)

    try:
        ssh_command(target, f"mkdir -p {shell_quote(remote_build_root)}")
        scp_upload(target, local_archive, remote_archive)
        remote_build_script = textwrap.dedent(
            f"""
            set -euo pipefail
            export PATH="$HOME/.cargo/bin:$PATH"

            rm -rf {shell_quote(remote_src_dir)}
            mkdir -p {shell_quote(remote_src_dir)}
            tar -xzf {shell_quote(remote_archive)} -C {shell_quote(remote_src_dir)}

            if ! command -v npm >/dev/null 2>&1; then
                echo "remote preflight failed: npm not found on PATH" >&2
                exit 1
            fi

            if ! command -v node >/dev/null 2>&1; then
                echo "remote preflight failed: node not found on PATH" >&2
                exit 1
            fi

            if ! command -v cargo >/dev/null 2>&1; then
                echo "remote preflight failed: cargo not found on PATH (expected ~/.cargo/bin/cargo)" >&2
                exit 1
            fi

            cd {shell_quote(remote_src_dir)}/frontend
            rm -rf node_modules
            rm -f package-lock.json
            npm install --include=optional

            node ./node_modules/vue-tsc/bin/vue-tsc.js --noEmit
            node ./node_modules/vite/bin/vite.js build

            cd {shell_quote(remote_src_dir)}
            cargo build --release -p obsidian-server
            cp {shell_quote(remote_src_dir + "/target/release/obsidian-host")} {shell_quote(remote_binary)}
            """
        ).strip()
        ssh_command(target, remote_build_script)

        local_binary = (
            REPO_ROOT / "target" / "remote-build" / target.name / "obsidian-host"
        )
        local_binary.parent.mkdir(parents=True, exist_ok=True)
        run_command(
            [
                "scp",
                "-P",
                str(target.ssh_port),
                f"{target.ssh_destination}:{remote_binary}",
                str(local_binary),
            ],
            cwd=REPO_ROOT,
        )
    finally:
        local_archive.unlink(missing_ok=True)
        try:
            ssh_command(target, f"rm -rf {shell_quote(remote_build_root)} || true")
        except DeployError:
            pass

    if not local_binary.exists():
        raise DeployError(
            f"Expected remote-built server binary not found locally: {local_binary}"
        )

    success(f"Backend built remotely and downloaded: {local_binary}")
    return local_binary


def build_supporting_crates() -> None:
    section("Building supporting crates")
    run_command(["cargo", "build", "-p", "obsidian-client"], cwd=REPO_ROOT)
    run_command(["cargo", "build", "-p", "obsidian-types"], cwd=REPO_ROOT)
    success("Supporting crates built successfully")


def render_remote_config(
    target: DeploymentTarget,
    source_config: Path = CONFIG_TEMPLATE,
    *,
    bootstrap_admin_username: str | None = None,
    bootstrap_admin_password: str | None = None,
) -> str:
    lines = source_config.read_text(encoding="utf-8").splitlines()
    rendered: list[str] = []
    current_section = ""
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            current_section = stripped.strip("[]")
            rendered.append(line)
            continue

        if current_section == "server" and stripped.startswith("host ="):
            rendered.append('host = "0.0.0.0"')
        elif current_section == "server" and stripped.startswith("port ="):
            rendered.append(f"port = {target.http_port}")
        elif current_section == "database" and stripped.startswith("path ="):
            rendered.append('path = "./obsidian-host.db"')
        elif current_section == "auth" and stripped.startswith("enabled ="):
            rendered.append("enabled = true")
        elif (
            current_section == "auth"
            and stripped.startswith("bootstrap_admin_username =")
            and bootstrap_admin_username is not None
        ):
            rendered.append(f'bootstrap_admin_username = "{bootstrap_admin_username}"')
        elif (
            current_section == "auth"
            and stripped.startswith("bootstrap_admin_password =")
            and bootstrap_admin_password is not None
        ):
            rendered.append(f'bootstrap_admin_password = "{bootstrap_admin_password}"')
        else:
            rendered.append(line)
    return "\n".join(rendered).strip() + "\n"


def generate_bootstrap_admin_credentials() -> tuple[str, str]:
    username = f"admin-{secrets.token_hex(4)}"
    alphabet = string.ascii_letters + string.digits + "-_."
    password = "".join(secrets.choice(alphabet) for _ in range(28))
    return username, password


def ensure_bootstrap_credentials_in_config(
    config_text: str,
) -> tuple[str, bool, str | None, str | None]:
    lines = config_text.splitlines()
    current_section = ""
    username_pattern = re.compile(
        r'^\s*bootstrap_admin_username\s*=\s*"(?P<value>[^"]*)"\s*(?:#.*)?$'
    )
    password_pattern = re.compile(
        r'^\s*bootstrap_admin_password\s*=\s*"(?P<value>[^"]*)"\s*(?:#.*)?$'
    )
    enabled_pattern = re.compile(r"^\s*enabled\s*=\s*(true|false)\s*(?:#.*)?$")

    username_index: int | None = None
    password_index: int | None = None
    enabled_index: int | None = None
    existing_username: str | None = None
    existing_password: str | None = None

    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            current_section = stripped.strip("[]")
            continue
        if current_section != "auth":
            continue

        username_match = username_pattern.match(line)
        if username_match:
            username_index = index
            existing_username = username_match.group("value")
            continue

        password_match = password_pattern.match(line)
        if password_match:
            password_index = index
            existing_password = password_match.group("value")
            continue

        if enabled_pattern.match(line):
            enabled_index = index

    if username_index is None or password_index is None:
        return config_text, False, None, None

    generated_username: str | None = None
    generated_password: str | None = None

    if not existing_username:
        generated_username, _ = generate_bootstrap_admin_credentials()
    if not existing_password:
        _, generated_password = generate_bootstrap_admin_credentials()

    if generated_username is None and generated_password is None:
        return config_text, False, None, None

    final_username = existing_username or generated_username
    final_password = existing_password or generated_password
    if final_username is None or final_password is None:
        return config_text, False, None, None

    lines[username_index] = f'bootstrap_admin_username = "{final_username}"'
    lines[password_index] = f'bootstrap_admin_password = "{final_password}"'
    if enabled_index is not None:
        lines[enabled_index] = "enabled = true"

    updated = "\n".join(lines)
    if config_text.endswith("\n"):
        updated += "\n"

    return updated, True, generated_username, generated_password


def render_systemd_unit(target: DeploymentTarget) -> str:
    template = SYSTEMD_TEMPLATE.read_text(encoding="utf-8")
    replacements = {
        "{{SERVICE_NAME}}": SERVICE_NAME,
        "{{SERVICE_USER}}": target.ssh_user,
        "{{APP_DIR}}": target.app_dir,
        "{{WORKING_DIRECTORY}}": target.shared_dir,
        "{{EXEC_START}}": f"{target.current_dir}/obsidian-host",
        "{{CONFIG_PATH}}": target.remote_config_path,
    }
    for placeholder, value in replacements.items():
        template = template.replace(placeholder, value)
    return template


def create_desktop_placeholder(dist_dir: Path, config_template_path: Path) -> Path:
    desktop_dir = dist_dir / "desktop"
    desktop_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(config_template_path, desktop_dir / "config.template.toml")
    (desktop_dir / "README.txt").write_text(
        textwrap.dedent(
            """
            Obsidian Host Desktop Placeholder
            =================================

            A native desktop executable is not available in the current workspace yet.
            This placeholder bundle exists so release packaging is stable and predictable.

            Included files:
            - config.template.toml: starter configuration you can reuse for future desktop builds

            Future desktop work is expected to live in a crate such as `crates/obsidian-desktop/`.
            """
        ).strip()
        + "\n",
        encoding="utf-8",
    )
    return desktop_dir


def assemble_dist(
    target: DeploymentTarget,
    *,
    build_support: bool = True,
    build_strategy: str = BUILD_STRATEGY_LOCAL,
) -> BuildArtifacts:
    if build_strategy == BUILD_STRATEGY_REMOTE:
        info("Using remote build strategy (build on target host)")
        server_binary = build_server_release_remote(target)
    else:
        build_frontend()
        server_binary = build_server_release()
        if build_support:
            build_supporting_crates()

    section("Assembling dist")
    if DIST_DIR.exists():
        shutil.rmtree(DIST_DIR)
    DIST_DIR.mkdir(parents=True, exist_ok=True)

    server_dir = DIST_DIR / "server"
    server_dir.mkdir(parents=True, exist_ok=True)
    binary_copy = server_dir / "obsidian-host"
    shutil.copy2(server_binary, binary_copy)

    config_template_path = DIST_DIR / "config.template.toml"
    shutil.copy2(CONFIG_TEMPLATE, config_template_path)

    remote_example_path = DIST_DIR / "server.config.example.toml"
    remote_example_path.write_text(render_remote_config(target), encoding="utf-8")

    desktop_dir = create_desktop_placeholder(DIST_DIR, config_template_path)

    git_commit = get_git_commit()
    server_sha = sha256_file(binary_copy)
    release_id = server_sha[:16]

    manifest = {
        "build_time_utc": datetime.now(timezone.utc).isoformat(),
        "git_commit": git_commit,
        "release_id": release_id,
        "target_platform": LINUX_SERVER_TARGET,
        "deployment_target": target.name,
        "server": {
            "artifact": str(binary_copy.relative_to(DIST_DIR)).replace("\\", "/"),
            "sha256": server_sha,
            "frontend_embedded": True,
        },
        "desktop": {
            "placeholder": True,
            "path": str(desktop_dir.relative_to(DIST_DIR)).replace("\\", "/"),
        },
    }
    manifest_path = DIST_DIR / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

    deploy_dir = DIST_DIR / "deploy"
    deploy_dir.mkdir(parents=True, exist_ok=True)
    tarball_path = deploy_dir / f"obsidian-host-{release_id}.tar.gz"
    with tarfile.open(tarball_path, "w:gz") as archive:
        archive.add(binary_copy, arcname="obsidian-host")
        archive.add(manifest_path, arcname="manifest.json")

    success(f"dist assembled at {DIST_DIR}")
    return BuildArtifacts(
        target_name=target.name,
        target_platform=LINUX_SERVER_TARGET,
        server_binary=binary_copy,
        server_binary_name=binary_copy.name,
        server_sha256=server_sha,
        dist_dir=DIST_DIR,
        manifest_path=manifest_path,
        tarball_path=tarball_path,
        config_template_path=config_template_path,
        desktop_placeholder_dir=desktop_dir,
        release_id=release_id,
        git_commit=git_commit,
    )


def load_existing_artifacts(target: DeploymentTarget) -> BuildArtifacts:
    manifest_path = DIST_DIR / "manifest.json"
    if not manifest_path.exists():
        raise DeployError("dist/manifest.json is missing. Run the build action first.")

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    server_relative = manifest["server"]["artifact"]
    server_binary = DIST_DIR / server_relative
    tarball_path = (
        DIST_DIR / "deploy" / f"obsidian-host-{manifest['release_id']}.tar.gz"
    )
    config_template_path = DIST_DIR / "config.template.toml"
    desktop_dir = DIST_DIR / "desktop"

    for required_path in [
        server_binary,
        tarball_path,
        config_template_path,
        desktop_dir,
    ]:
        if not required_path.exists():
            raise DeployError(f"Expected dist artifact missing: {required_path}")

    return BuildArtifacts(
        target_name=target.name,
        target_platform=manifest.get("target_platform", LINUX_SERVER_TARGET),
        server_binary=server_binary,
        server_binary_name=server_binary.name,
        server_sha256=manifest["server"]["sha256"],
        dist_dir=DIST_DIR,
        manifest_path=manifest_path,
        tarball_path=tarball_path,
        config_template_path=config_template_path,
        desktop_placeholder_dir=desktop_dir,
        release_id=manifest["release_id"],
        git_commit=manifest.get("git_commit", "unknown"),
    )


def ssh_command(
    target: DeploymentTarget, command: str, *, capture_output: bool = False
) -> subprocess.CompletedProcess[str]:
    remote_bash_command = f"bash -lc {shell_quote(command)}"
    return run_command(
        [
            "ssh",
            "-p",
            str(target.ssh_port),
            target.ssh_destination,
            remote_bash_command,
        ],
        cwd=REPO_ROOT,
        capture_output=capture_output,
    )


def scp_upload(target: DeploymentTarget, local_path: Path, remote_path: str) -> None:
    run_command(
        [
            "scp",
            "-P",
            str(target.ssh_port),
            str(local_path),
            f"{target.ssh_destination}:{remote_path}",
        ],
        cwd=REPO_ROOT,
    )


def remote_file_sha(target: DeploymentTarget, remote_path: str) -> str | None:
    result = ssh_command(
        target,
        f"if [ -f {shell_quote(remote_path)} ]; then sha256sum {shell_quote(remote_path)} | awk '{{print $1}}'; fi",
        capture_output=True,
    )
    output = result.stdout.strip()
    return output or None


def remote_path_exists(target: DeploymentTarget, remote_path: str) -> bool:
    result = ssh_command(
        target,
        f"if [ -e {shell_quote(remote_path)} ]; then echo yes; else echo no; fi",
        capture_output=True,
    )
    return result.stdout.strip() == "yes"


def remote_readlink(target: DeploymentTarget, remote_path: str) -> str:
    result = ssh_command(
        target,
        f"if [ -L {shell_quote(remote_path)} ] || [ -e {shell_quote(remote_path)} ]; then readlink -f {shell_quote(remote_path)}; fi",
        capture_output=True,
    )
    return result.stdout.strip()


def shell_quote(value: str) -> str:
    return "'" + value.replace("'", "'\"'\"'") + "'"


def ensure_remote_layout(target: DeploymentTarget) -> None:
    section("Ensuring remote layout")
    ssh_command(
        target,
        " && ".join(
            [
                f"sudo mkdir -p {shell_quote(target.releases_dir)}",
                f"sudo mkdir -p {shell_quote(target.shared_dir)}",
                f"sudo mkdir -p {shell_quote(target.tmp_dir)}",
                f"sudo chown -R {shell_quote(target.ssh_user + ':' + target.ssh_user)} {shell_quote(target.app_dir)}",
            ]
        ),
    )
    success("Remote directories are ready")


def upload_release_if_needed(
    target: DeploymentTarget, artifacts: BuildArtifacts
) -> bool:
    release_dir = f"{target.releases_dir}/{artifacts.release_id}"
    release_binary = f"{release_dir}/obsidian-host"
    if remote_path_exists(target, release_binary):
        info(f"Remote release {artifacts.release_id} already exists; skipping upload")
        return False

    section("Uploading release bundle")
    remote_tarball = f"{target.tmp_dir}/{artifacts.tarball_path.name}"
    scp_upload(target, artifacts.tarball_path, remote_tarball)
    ssh_command(
        target,
        " && ".join(
            [
                f"mkdir -p {shell_quote(release_dir)}",
                f"tar -xzf {shell_quote(remote_tarball)} -C {shell_quote(release_dir)}",
                f"chmod +x {shell_quote(release_binary)}",
                f"rm -f {shell_quote(remote_tarball)}",
            ]
        ),
    )
    success(f"Uploaded release {artifacts.release_id}")
    return True


def ensure_remote_config(
    target: DeploymentTarget,
) -> tuple[bool, bool, str | None, str | None]:
    if remote_path_exists(target, target.remote_config_path):
        result = ssh_command(
            target,
            f"cat {shell_quote(target.remote_config_path)}",
            capture_output=True,
        )
        existing_config = result.stdout
        (
            updated_config,
            config_updated,
            generated_username,
            generated_password,
        ) = ensure_bootstrap_credentials_in_config(existing_config)

        if not config_updated:
            info(f"Remote config exists at {target.remote_config_path}; preserving it")
            return False, False, None, None

        section("Updating remote config bootstrap admin")
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", delete=False, suffix=".toml"
        ) as handle:
            handle.write(updated_config)
            temp_path = Path(handle.name)

        try:
            remote_tmp = f"{target.tmp_dir}/config.toml"
            scp_upload(target, temp_path, remote_tmp)
            ssh_command(
                target,
                " && ".join(
                    [
                        f"mv {shell_quote(remote_tmp)} {shell_quote(target.remote_config_path)}",
                        f"chmod 640 {shell_quote(target.remote_config_path)}",
                    ]
                ),
            )
        finally:
            temp_path.unlink(missing_ok=True)

        success(f"Updated remote config at {target.remote_config_path}")
        warn("Generated initial admin credentials (shown only this provisioning run):")
        if generated_username:
            print(f"  username: {generated_username}")
        if generated_password:
            print(f"  password: {generated_password}")
        return False, True, generated_username, generated_password

    section("Creating remote config")
    admin_username, admin_password = generate_bootstrap_admin_credentials()
    config_text = render_remote_config(
        target,
        bootstrap_admin_username=admin_username,
        bootstrap_admin_password=admin_password,
    )
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", delete=False, suffix=".toml"
    ) as handle:
        handle.write(config_text)
        temp_path = Path(handle.name)

    try:
        remote_tmp = f"{target.tmp_dir}/config.toml"
        scp_upload(target, temp_path, remote_tmp)
        ssh_command(
            target,
            " && ".join(
                [
                    f"mv {shell_quote(remote_tmp)} {shell_quote(target.remote_config_path)}",
                    f"chmod 640 {shell_quote(target.remote_config_path)}",
                ]
            ),
        )
    finally:
        temp_path.unlink(missing_ok=True)

    success(f"Created remote config at {target.remote_config_path}")
    warn("Generated initial admin credentials (shown only this provisioning run):")
    print(f"  username: {admin_username}")
    print(f"  password: {admin_password}")
    return True, False, admin_username, admin_password


def ensure_systemd_unit(target: DeploymentTarget) -> bool:
    section("Ensuring systemd unit")
    unit_text = render_systemd_unit(target)
    local_hash = hashlib.sha256(unit_text.encode("utf-8")).hexdigest()
    remote_hash = remote_file_sha(target, target.systemd_unit_path)
    if remote_hash == local_hash:
        info("Remote systemd unit is already up to date")
        return False

    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", delete=False, suffix=".service"
    ) as handle:
        handle.write(unit_text)
        temp_path = Path(handle.name)

    try:
        remote_tmp = f"{target.tmp_dir}/{SERVICE_NAME}.service"
        scp_upload(target, temp_path, remote_tmp)
        ssh_command(
            target,
            " && ".join(
                [
                    f"sudo mv {shell_quote(remote_tmp)} {shell_quote(target.systemd_unit_path)}",
                    f"sudo chmod 644 {shell_quote(target.systemd_unit_path)}",
                ]
            ),
        )
    finally:
        temp_path.unlink(missing_ok=True)

    success("Systemd unit updated")
    return True


def activate_release(target: DeploymentTarget, artifacts: BuildArtifacts) -> bool:
    release_dir = f"{target.releases_dir}/{artifacts.release_id}"
    current_target = remote_readlink(target, target.current_dir)
    if current_target == release_dir:
        info(f"Current release already points at {release_dir}")
        return False

    section("Activating release")
    ssh_command(
        target, f"ln -sfn {shell_quote(release_dir)} {shell_quote(target.current_dir)}"
    )
    success(f"Activated release {artifacts.release_id}")
    return True


def ensure_service_state(
    target: DeploymentTarget,
    *,
    unit_updated: bool,
    release_activated: bool,
    config_created: bool,
) -> tuple[bool, bool]:
    section("Ensuring service state")
    if unit_updated:
        ssh_command(target, "sudo systemctl daemon-reload")

    enable_result = ssh_command(
        target,
        f"sudo systemctl is-enabled {shell_quote(SERVICE_NAME)} >/dev/null 2>&1 || sudo systemctl enable {shell_quote(SERVICE_NAME)}",
        capture_output=False,
    )
    service_enabled = enable_result.returncode == 0  # unreachable if non-zero

    should_restart = unit_updated or release_activated or config_created
    if should_restart:
        ssh_command(target, f"sudo systemctl restart {shell_quote(SERVICE_NAME)}")
        success("Service restarted")
        return service_enabled, True

    ssh_command(
        target,
        f"sudo systemctl is-active {shell_quote(SERVICE_NAME)} >/dev/null 2>&1 || sudo systemctl start {shell_quote(SERVICE_NAME)}",
    )
    success("Service already current; ensured it is running")
    return service_enabled, False


def wait_for_healthcheck(target: DeploymentTarget, timeout_seconds: int = 30) -> bool:
    section("Running health check")
    url = f"http://{target.ip_address}:{target.http_port}/"
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=5) as response:
                if response.status == 200:
                    success(f"Health check passed: {url}")
                    return True
        except (urllib.error.URLError, TimeoutError):
            time.sleep(2)
    warn(f"Health check did not pass before timeout: {url}")
    return False


def deploy_to_target(
    target: DeploymentTarget, artifacts: BuildArtifacts
) -> DeployResult:
    ensure_tool_exists("ssh")
    ensure_tool_exists("scp")
    ensure_remote_layout(target)
    release_uploaded = upload_release_if_needed(target, artifacts)
    (
        config_created,
        config_updated,
        generated_admin_username,
        generated_admin_password,
    ) = ensure_remote_config(target)
    unit_updated = ensure_systemd_unit(target)
    release_activated = activate_release(target, artifacts)
    service_enabled, service_restarted = ensure_service_state(
        target,
        unit_updated=unit_updated,
        release_activated=release_activated,
        config_created=(config_created or config_updated),
    )
    healthcheck_ok = wait_for_healthcheck(target)
    return DeployResult(
        release_uploaded=release_uploaded,
        release_activated=release_activated,
        config_created=config_created,
        config_updated=config_updated,
        unit_updated=unit_updated,
        service_restarted=service_restarted,
        service_enabled=service_enabled,
        healthcheck_ok=healthcheck_ok,
        generated_admin_username=generated_admin_username,
        generated_admin_password=generated_admin_password,
    )


def run_preflight_checks(
    target: DeploymentTarget, build_strategy: str
) -> list[PreflightCheck]:
    checks: list[PreflightCheck] = []

    def add_check(name: str, ok: bool, detail: str) -> None:
        checks.append(PreflightCheck(name=name, ok=ok, detail=detail))

    def local_tool_check(tool_name: str) -> None:
        path = shutil.which(tool_name)
        add_check(
            f"local:{tool_name}",
            path is not None,
            path or "not found on PATH",
        )

    local_tool_check("ssh")
    local_tool_check("scp")

    if build_strategy == BUILD_STRATEGY_LOCAL:
        local_tool_check("npm")
        local_tool_check("cargo")
        if platform.system().lower() != "linux":
            local_tool_check("cross")
            local_tool_check("docker")
            if shutil.which("docker") is not None:
                try:
                    run_command(
                        [
                            "docker",
                            "version",
                            "-f",
                            "{{ .Server.Os }},,,{{ .Server.Arch }}",
                        ],
                        cwd=REPO_ROOT,
                        capture_output=True,
                    )
                    add_check("local:docker-daemon", True, "reachable")
                except DeployError as error:
                    add_check("local:docker-daemon", False, str(error))
            else:
                add_check("local:docker-daemon", False, "docker not found on PATH")

    try:
        result = ssh_command(target, "echo preflight-ok", capture_output=True)
        reachable = result.stdout.strip() == "preflight-ok"
        add_check(
            "remote:ssh-connectivity",
            reachable,
            "connected" if reachable else "unexpected ssh response",
        )
    except DeployError as error:
        add_check("remote:ssh-connectivity", False, str(error))
        return checks

    remote_probes = [
        ("bash", "command -v bash >/dev/null 2>&1"),
        ("tar", "command -v tar >/dev/null 2>&1"),
        ("npm", "command -v npm >/dev/null 2>&1"),
        ("cargo", "command -v cargo >/dev/null 2>&1"),
        ("systemctl", "command -v systemctl >/dev/null 2>&1"),
    ]
    for name, command in remote_probes:
        try:
            ssh_command(target, command, capture_output=True)
            add_check(f"remote:{name}", True, "available")
        except DeployError as error:
            add_check(f"remote:{name}", False, str(error))

    try:
        ssh_command(target, "sudo -n true >/dev/null 2>&1", capture_output=True)
        add_check("remote:sudo-nopasswd", True, "enabled")
    except DeployError as error:
        add_check("remote:sudo-nopasswd", False, str(error))

    return checks


def print_preflight_summary(
    target: DeploymentTarget,
    build_strategy: str,
    checks: list[PreflightCheck],
) -> None:
    section("Preflight summary")
    print(f"target={target.name}")
    print(f"build_strategy={build_strategy}")
    for check in checks:
        status = "PASS" if check.ok else "FAIL"
        print(f"- [{status}] {check.name}: {check.detail}")

    failures = sum(1 for check in checks if not check.ok)
    print(f"\nchecks={len(checks)} failures={failures}")


def run_preflight(target: DeploymentTarget, build_strategy: str) -> int:
    checks = run_preflight_checks(target, build_strategy)
    print_preflight_summary(target, build_strategy, checks)
    if any(not check.ok for check in checks):
        warn("Preflight failed. Resolve failed checks before deployment.")
        return 1
    success("Preflight passed. Ready to build/deploy.")
    return 0


def print_target_table(targets: Iterable[DeploymentTarget]) -> None:
    section("Targets")
    for target in targets:
        print(
            f"- {target.name}: {target.ssh_user}@{target.ssh_host}:{target.ssh_port} | "
            f"http://{target.ip_address}:{target.http_port} | app_dir={target.app_dir}"
        )


def print_artifact_summary(artifacts: BuildArtifacts) -> None:
    section("Artifact summary")
    summary = {
        "target_name": artifacts.target_name,
        "release_id": artifacts.release_id,
        "git_commit": artifacts.git_commit,
        "server_binary": str(artifacts.server_binary),
        "manifest_path": str(artifacts.manifest_path),
        "tarball_path": str(artifacts.tarball_path),
        "desktop_placeholder_dir": str(artifacts.desktop_placeholder_dir),
    }
    print(json.dumps(summary, indent=2))


def print_deploy_summary(result: DeployResult) -> None:
    section("Deploy summary")
    print(json.dumps(asdict(result), indent=2))


def resolve_target(
    name: str | None, targets: dict[str, DeploymentTarget]
) -> DeploymentTarget:
    if name:
        try:
            return targets[name]
        except KeyError as exc:
            raise DeployError(f"Unknown target '{name}'") from exc
    return choose_target_interactively(targets)


def run_interactive_menu(targets: dict[str, DeploymentTarget]) -> int:
    target = choose_target_interactively(targets)
    artifacts: BuildArtifacts | None = None
    menu_build_strategy = BUILD_STRATEGY_LOCAL

    while True:
        section(f"Deployment menu ({target.name})")
        print("1. Build release artifacts")
        print("2. Deploy existing dist artifacts")
        print("3. Build and deploy")
        print("4. Show target details")
        print("5. Exit")
        choice = input("Choose an action: ").strip()

        try:
            if choice == "1":
                artifacts = assemble_dist(target, build_strategy=menu_build_strategy)
                print_artifact_summary(artifacts)
            elif choice == "2":
                artifacts = artifacts or load_existing_artifacts(target)
                result = deploy_to_target(target, artifacts)
                print_deploy_summary(result)
            elif choice == "3":
                artifacts = assemble_dist(target, build_strategy=menu_build_strategy)
                print_artifact_summary(artifacts)
                result = deploy_to_target(target, artifacts)
                print_deploy_summary(result)
            elif choice == "4":
                print(json.dumps(asdict(target), indent=2))
            elif choice == "5":
                return 0
            else:
                warn("Please choose 1-5")
        except DeployError as error:
            warn(str(error))


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Build and deploy obsidian-host to remote VMs"
    )
    parser.add_argument(
        "--targets-file", type=Path, default=TARGETS_FILE, help="Path to targets.toml"
    )
    subparsers = parser.add_subparsers(dest="command")

    subparsers.add_parser("targets", help="List deployment targets")

    doctor_parser = subparsers.add_parser(
        "doctor", help="Run deployment preflight checks"
    )
    doctor_parser.add_argument("--target", help="Target name from targets.toml")
    doctor_parser.add_argument(
        "--remote-build",
        action="store_true",
        help="Check remote-build prerequisites explicitly instead of the default local build flow",
    )

    build_parser = subparsers.add_parser("build", help="Build dist artifacts")
    build_parser.add_argument("--target", help="Target name from targets.toml")
    build_parser.add_argument(
        "--skip-support-builds",
        action="store_true",
        help="Skip cargo build checks for obsidian-client and obsidian-types",
    )
    build_parser.add_argument(
        "--remote-build",
        action="store_true",
        help="Build frontend/backend on target VM instead of the default local build flow",
    )

    deploy_parser = subparsers.add_parser(
        "deploy", help="Deploy using current dist or rebuild first"
    )
    deploy_parser.add_argument("--target", help="Target name from targets.toml")
    deploy_parser.add_argument(
        "--build-first",
        action="store_true",
        help="Rebuild dist artifacts before deploying",
    )
    deploy_parser.add_argument(
        "--remote-build",
        action="store_true",
        help="When used with --build-first, build frontend/backend on target VM instead of locally",
    )

    build_deploy_parser = subparsers.add_parser(
        "build-and-deploy", help="Build dist artifacts and deploy"
    )
    build_deploy_parser.add_argument("--target", help="Target name from targets.toml")
    build_deploy_parser.add_argument(
        "--skip-support-builds",
        action="store_true",
        help="Skip cargo build checks for obsidian-client and obsidian-types",
    )
    build_deploy_parser.add_argument(
        "--remote-build",
        action="store_true",
        help="Build frontend/backend on target VM and then deploy, instead of using the default local build flow",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_arg_parser()
    args = parser.parse_args(argv)
    targets = load_targets(args.targets_file)

    if not args.command:
        return run_interactive_menu(targets)

    if args.command == "targets":
        print_target_table(targets.values())
        return 0

    target = resolve_target(getattr(args, "target", None), targets)
    build_strategy = BUILD_STRATEGY_LOCAL
    if getattr(args, "remote_build", False):
        build_strategy = BUILD_STRATEGY_REMOTE

    try:
        if args.command == "doctor":
            return run_preflight(target, build_strategy)

        if args.command == "build":
            artifacts = assemble_dist(
                target,
                build_support=not args.skip_support_builds,
                build_strategy=build_strategy,
            )
            print_artifact_summary(artifacts)
            return 0

        if args.command == "deploy":
            artifacts = (
                assemble_dist(target, build_strategy=build_strategy)
                if args.build_first
                else load_existing_artifacts(target)
            )
            result = deploy_to_target(target, artifacts)
            print_deploy_summary(result)
            return 0

        if args.command == "build-and-deploy":
            artifacts = assemble_dist(
                target,
                build_support=not args.skip_support_builds,
                build_strategy=build_strategy,
            )
            print_artifact_summary(artifacts)
            result = deploy_to_target(target, artifacts)
            print_deploy_summary(result)
            return 0
    except DeployError as error:
        warn(str(error))
        return 1

    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
