# Advanced Features Design Document

This document outlines the design and implementation plans for advanced features in Obsidian Host, covering Task 12.2.

## 1. Graph View Data Structure

The Graph View visualizes the connections between notes, attachments, and tags.

### Data Model
The graph is represented as a collection of nodes and edges (see `src/models/graph.rs` for implementation).

**Nodes:**
- **Files**: Markdown files, images, PDFs.
- **Tags**: Hashtags found within content.
- **Virtual**: Links to files that don't satisfy existence checks (broken links).

**Edges:**
- **Links**: Wiki-links `[[note]]`.
- **Embeds**: Embeds `![[image.png]]`.
- **Tag Usage**: File contains `#tag`.

### Storage & Serialization
- Graph data is computed on-demand or incrementally updated.
- Serialized as JSON for consumption by the frontend (D3.js or Cytoscape.js).
- Caching strategy: The computed graph structure can be cached in memory and invalidated on file change events.

## 2. Mobile Responsive Design

The application must be fully usable on mobile devices.

### Strategy
- **Breakpoints**:
    - Mobile: < 640px
    - Tablet: 640px - 1024px
    - Desktop: > 1024px
- **Layout Adjustments**:
    - **Sidebar**: Collapsed by default on mobile, accessible via hamburger menu.
    - **Editor/Preview**: Single column view on mobile (switchable tabs) vs Side-by-Side on desktop.
    - **Modals**: Full-screen on mobile.
- **Touch Targets**: Min height 44px for clickable elements.
- **Input Handling**: Virtual keyboard considerations (avoid covering inputs).

## 3. Authentication System

To support multi-user environments/hosting, authentication is required.

### Phases
1.  **Single User Password Protection**: Simple shared password (basic auth or form-based).
2.  **User Accounts**: Full user database with specialized roles (Admin, Editor, Viewer).

### Implementation Plan
- **Identity Provider**: Built-in SQLite user table or potential OIDC integration.
- **Sessions**: JWT (JSON Web Tokens) stored in HTTP-only cookies.
- **Vault Access**: Users assigned permissions per vault (Read, Write, Admin).

## 4. Multi-user Support

Enabling real-time collaboration.

### Conflict Resolution Strategy
1.  **Last Write Wins (Current)**: Simple, but risky for simultaneous edits.
2.  **Operational Transformation (OT) / CRDTs (Future)**:
    - Shift to CRDTs (Conflict-free Replicated Data Types) for text content.
    - Use libraries like `yjs` (WASM) or `automerge`.
    - Backend acts as a relay for operation messages via WebSocket.

### Presence
- Show "Who is viewing this note" in the UI.
- Cursor tracking (remote cursors).

## 5. Version Control Integration

Providing history and backup capabilities.

### Git Integration
- **Internal Git**: Treat each vault as a Git repository.
- **Auto-commit**: Commit changes on file save or periodically.
- **UI**:
    - History view (git log).
    - Diff view (git diff).
    - Restore/Revert specific commits.
- **Sync**: Push/Pull to remote repositories (GitHub/GitLab) for backup.

## 6. Extension Points

Allowing plugins to extend functionality.

### Plugin API
- **Frontend**: JavaScript/WASM plugins loaded at runtime.
    - Hooks: `onLoad`, `onUnload`.
    - UI Injection: Add buttons to headers, sidebar panels, context menus.
    - Editor: CodeMirror extensions.
- **Backend**: (Harder to secure)
    - Maybe WASM plugins (using Extism or Wasmtime) for server-side logic (custom parsers, search filters).

### Lifecycle
1.  **Discovery**: Scan `plugins/` directory.
2.  **Manifest**: `plugin.json` defining capabilities.
3.  **Loading**: Safe sandbox loading.
