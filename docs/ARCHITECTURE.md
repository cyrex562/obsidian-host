# Architecture Overview

Obsidian Host is a full-stack application built with Rust (backend) and TypeScript (frontend), managing a local SQLite database for metadata.

## Tech Stack

### Backend
-   **Language**: Rust (Edition 2021)
-   **Web Framework**: Actix Web
-   **Database**: SQLx (SQLite)
-   **Search**: In-memory inverted index (custom implementation)
-   **File Watching**: `notify` (Cross-platform filesystem events)
-   **Markdown**: `pulldown-cmark`
-   **Template Engine**: None (API only, frontend is SPA)

### Frontend
-   **Language**: TypeScript
-   **Build Tool**: `tsc` (Simple custom build script)
-   **Rendering**: HTML5 / Vanilla JS / Web Components (CodeJar editor)
-   **Styling**: CSS Variables, Dark/Light mode support

## Core Components

### 1. `AppConfig` & `Database`
-   Application configuration is loaded from `config.toml`, ENV variables, and defaults.
-   SQLite stores:
    -   `vaults`: Registered vault paths.
    -   `preferences`: User settings.
    -   `recent_files`: History.

### 2. `FileService`
-   Handles all filesystem I/O.
-   Performs security checks (Path Traversal prevention) using `canonicalize`.
-   Operations: Read, Write, Create (recursive), Delete (move to `.trash`), Move/Rename.

### 3. `SearchIndex`
-   An in-memory structure mapping tokens to file paths and line numbers.
-   Built on startup by scanning registered vaults.
-   Updated incrementally via file events.
-   Provides fast full-text search.

### 4. `FileWatcher`
-   Runs in a separate thread.
-   Watches all registered vault paths recursively.
-   Debounces events to prevent floods.
-   Broadcasts events (`Created`, `Modified`, `Deleted`, `Renamed`) via a Tokio broadcast channel.

### 5. `WebSocketHandler`
-   Accepts WebSocket connections from the frontend.
-   Subscribes to the file event broadcast channel.
-   Pushes updates to clients to trigger UI refreshes (e.g., file tree update, content reload).

## Data Flow

1.  **User Edit**: Frontend sends `PUT /api/files/...`.
2.  **API Handler**: `FileService` writes to disk.
3.  **Filesystem**: OS confirms write.
4.  **Watcher**: Detects `Modify` event.
5.  **Event Loop**:
    -   Updates `SearchIndex`.
    -   Broadcasts event to WebSockets.
6.  **Frontend**: Receives event.
    -   If file is open elsewhere, warns user or updates.
    -   If file tree changed, re-fetches tree.

## Directory Structure

-   `src/`: Backend Rust code
    -   `config/`: Configuration logic
    -   `db/`: Database migrations and repositories
    -   `models/`: API and DB structs
    -   `routes/`: Actix request handlers
    -   `services/`: Core business logic (File, Search, Markdown)
-   `frontend/`: Frontend TypeScript code
    -   `src/`: TS source
    -   `public/`: Static assets (HTML, CSS)
