# Configuration Guide

Obsidian Host can be configured via a `config.toml` file or environment variables.

## File Configuration (`config.toml`)

Place a file named `config.toml` in the running directory.

```toml
[server]
host = "0.0.0.0"  # Listen address
port = 8080       # Listen port

[database]
path = "./obsidian-host.db" # Path to SQLite database

[vault]
index_exclusions = [".git", ".obsidian", ".trash", "node_modules"] # Folders to ignore
```

## Environment Variables

Environment variables override file settings. Use double underscores `__` for nesting.

| Variable | Config Option | Default | Description |
|----------|---------------|---------|-------------|
| `OBSIDIAN__SERVER__HOST` | `server.host` | `127.0.0.1` | Binding address |
| `OBSIDIAN__SERVER__PORT` | `server.port` | `8080` | Binding port |
| `OBSIDIAN__DATABASE__PATH` | `database.path` | `./obsidian-host.db` | Database file location |
| `RUST_LOG` | N/A | `warn` | Logging verbosity (error, warn, info, debug, trace) |

## Logging
Logging is configured via `RUST_LOG`.
-   **JSON Format**: Set `LOG_FORMAT=json` for structured logging (useful for clouds).
-   **File Logging**: Logs are automatically rotated in `./logs/`.
