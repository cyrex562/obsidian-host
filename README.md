# Obsidian Host

A self-hosted web UI for Obsidian vaults built with Rust and HTMX.

## Features

- **Multi-vault support**: Manage multiple Obsidian vaults from a single interface
- **File management**: Browse, create, edit, and delete files and folders
- **Real-time sync**: Two-way synchronization between filesystem and web UI using file watching
- **Conflict resolution**: Automatic conflict detection with backup file creation
- **Full-text search**: Fast search across all markdown files with indexed content
- **Multiple editor modes**:
  - Raw markdown editor
  - Side-by-side (editor + live preview)
  - Formatted raw (with syntax highlighting)
  - Fully rendered view
- **Obsidian syntax support**: Wiki links, embeds, tags, and frontmatter
- **Split view**: Work with multiple files simultaneously in split panes
- **Tab management**: Open multiple files with tab interface
- **Dark/Light themes**: Toggle between themes for comfortable viewing

## Tech Stack

### Backend
- **Rust** with actix-web framework
- **SQLite** for metadata storage
- **notify** crate for file system watching
- **pulldown-cmark** for markdown parsing
- **Full-text search** with in-memory indexing

### Frontend
- **HTMX** for dynamic interactions
- **TypeScript** for type-safe client code
- **WebSocket** for real-time file change notifications
- **Pure CSS** for styling (no framework dependencies)

## Documentation

-   [User Guide](docs/USER_GUIDE.md) - How to use the interface.
-   [Installation & Build](docs/BUILD.md) - Building from source and cross-compilation.
-   [Docker Deployment](docs/DOCKER.md) - Running with Docker.
-   [API Reference](docs/API.md) - API endpoints documentation.
-   [Architecture](docs/ARCHITECTURE.md) - System overview.
-   [Configuration](docs/CONFIGURATION.md) - Config file and env vars.
-   [Future Plans](docs/ADVANCED_FEATURES.md) - Roadmap and advanced features.

## Quick Start
1.  **Download** the latest release or build from source (see [Build Guide](docs/BUILD.md)).
2.  **Run** the binary or Docker container.
3.  **Open** `http://localhost:8080`.
4.  **Register** your local vault path.

For detailed configuration options, see the [User Guide](docs/USER_GUIDE.md) or checks `config.toml`.

## How It Works

- File watching with `notify` crate monitors vault changes
- WebSocket broadcasts updates to connected clients
- Automatic conflict resolution with backup file creation
- Real-time search index updates
- Path traversal protection for security

## API Endpoints

- `GET /api/vaults` - List vaults
- `POST /api/vaults` - Create vault
- `GET /api/vaults/{vault_id}/files` - Get file tree
- `GET /api/vaults/{vault_id}/files/{path}` - Read file
- `PUT /api/vaults/{vault_id}/files/{path}` - Update file
- `GET /api/vaults/{vault_id}/search?q={query}` - Search
- `GET /api/ws` - WebSocket for real-time updates

See PROJECT_PLAN.md for detailed feature breakdown and implementation status.

## License

MIT
