# Docker Deployment Guide

## Prerequisites
- Docker Engine installed
- Docker Compose (optional but recommended)

## Building the Image

To build the Docker image locally:

```bash
docker build -t obsidian-host .
```

*Note: The build process is multi-stage and builds both the frontend and backend. It may take a few minutes initially.*

## Running with Docker Compose

We provide a `docker-compose.yml` for easy deployment.

1.  Inspect and modify `docker-compose.yml` if needed (e.g., ports, volumes).
2.  Start the service:
    ```bash
    docker-compose up -d
    ```

The application will be available at `http://localhost:8080`.

## Running with Docker CLI

```bash
docker run -d \
  --name obsidian-host \
  -p 8080:8080 \
  -v obsidian_data:/data \
  obsidian-host
```

## Volumes

The container uses `/data` to store:
- `obsidian-host.db` (The SQLite database)
- `vaults/` (Default location for created vaults, though you can mount external paths)

### Mounting External Vaults

To make your local vaults accessible to the container, mount them to `/data/vaults` or any other path inside the container:

```bash
docker run -d \
  -p 8080:8080 \
  -v $(pwd)/my-local-vaults:/data/vaults \
  -v obsidian_data:/data \
  obsidian-host
```

Then, when registering a vault in the UI, use the container path (e.g., `/data/vaults/my-vault`).

## Configuration

You can override configuration using environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `OBSIDIAN__SERVER__PORT` | 8080 | Server listening port |
| `OBSIDIAN__DATABASE__PATH` | `/data/obsidian-host.db` | Path to SQLite database |
| `RUST_LOG` | `info` | Logging level (error, warn, info, debug, trace) |

## Image optimization

The Dockerfile uses a multistage build to keep the image size small:
1.  **Builder (Node)**: Compiles frontend assets.
2.  **Builder (Rust)**: Compiles the backend binary.
3.  **Runtime (Debian Slim)**: Only contains the compiled binary and minimal runtime deps.

This results in a significantly smaller image than one containing the full toolchain.
