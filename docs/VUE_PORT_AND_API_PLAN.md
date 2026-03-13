# Plan: Vue Frontend Port + REST API for Native Desktop Client

**Status**: Phases A-C largely implemented; Phase D onward not started  
**Date created**: 2026-03-11  
**Decisions**:

- Vue 3 Composition API + Pinia + Vue Router 4
- Vuetify 3 component library (dark Obsidian theme)
- Pure native Rust GUI target (egui / iced) using a shared `obsidian-client` crate
- Keep existing `/api/` URL paths (no versioning prefix)
- JWT auth with config opt-out (`auth.enabled = false`) for local installs

**TL;DR**: Replace the vanilla TS + tsc frontend with a **Vite + Vue 3 + Pinia + Vuetify** app. Update `rust-embed` to point at Vite's output directory. Then convert Cargo to a **workspace** with a shared `obsidian-types` crate and a typed `obsidian-client` crate, add JWT auth, and expose delta-sync endpoints for the future egui/iced desktop app.

---

## Phase A — Vue Project Scaffolding

> Steps 1–5 are sequential.

- [x] **Step 1**: Replace `frontend/package.json` with Vite + Vue 3 deps:
  - Runtime: `vue`, `pinia`, `vue-router@4`, `vuetify@3`, `@mdi/font`
  - Build: `@vitejs/plugin-vue`, `vite`
  - Keep: `vitest`, `@playwright/test`, `typescript`
  - Remove: `codejar` (moved to vendor), CDN deps (htmx, quill, pdf.js — replaced by components)

- [x] **Step 2**: Create `frontend/vite.config.ts`:
  - `plugins: [vue()]`
  - `build.outDir: "../target/frontend"` (matches rust-embed path convention)
  - `base: "/"`
  - `server.proxy: { "/api": "http://localhost:3000" }` for dev HMR

- [x] **Step 3**: Update `frontend/tsconfig.json`:
  - `"moduleResolution": "bundler"`
  - `"jsx": "preserve"` (Vue SFC support)
  - Add Vue shims type reference

- [x] **Step 4**: Update `src/assets.rs`:

  ```rust
  // Change from (legacy):
  #[folder = "frontend/public/"]
  // To:
  #[folder = "target/frontend/"]
  ```

- [x] **Step 5**: Update `src/main.rs` debug-mode static serve path:

  ```rust
  // Change from (legacy):
  fs::Files::new("/", "./frontend/public").index_file("index.html")
  // To:
  fs::Files::new("/", "./target/frontend").index_file("index.html")
  ```

  (In development, run `vite build --watch` in `frontend/` alongside `cargo run`)

---

## Phase B — Pinia Stores + API Layer

> Steps 6–14 can be developed in parallel within this phase.

- [x] **Step 6**: Create `frontend/src/api/client.ts`
  - Typed `fetch` wrapper (or `axios` if preferred)
  - Auto-attach `Authorization: Bearer <token>` header from auth store
  - Centralized error handling (throws typed `ApiError`)
  - Methods mirror the existing `ApiClient` class in `frontend/src/app.ts`

- [x] **Step 7**: Create `frontend/src/api/types.ts`
  - TypeScript interfaces matching Rust models exactly:
    `Vault`, `FileNode`, `FileContent`, `SearchResult`, `SearchMatch`,
    `UserPreferences`, `FileChangeEvent`, `WsMessage`
  - Also: `LoginRequest`, `LoginResponse`, `SyncRequest`, `SyncResponse`

- [x] **Step 8**: Create `frontend/src/stores/auth.ts`
  - State: `accessToken`, `refreshToken`, `expiresAt`, `user`
  - Actions: `login(user, pass)`, `logout()`, `refreshToken()`
  - Persist token to `localStorage` (or `sessionStorage`)

- [x] **Step 9**: Create `frontend/src/stores/vaults.ts`
  - State: `vaults: Vault[]`, `activeVaultId: string | null`
  - Actions: `loadVaults()`, `createVault()`, `deleteVault()`, `setActive(id)`

- [x] **Step 10**: Create `frontend/src/stores/files.ts`
  - State: `tree: FileNode[]`, `loading: boolean`
  - Actions: `loadTree(vaultId)`, `createFile()`, `deleteFile()`, `renameFile()`, `createDirectory()`

- [x] **Step 11**: Create `frontend/src/stores/tabs.ts`
  - Port `AppState.openTabs`, `panes`, `activePaneId`, `splitOrientation` from `frontend/src/app.ts`
  - State: `tabs: Map<string, Tab>`, `panes: Pane[]`, `activePaneId`
  - Actions: `openTab()`, `closeTab()`, `splitPane()`, `mergePane()`

- [x] **Step 12**: Create `frontend/src/stores/editor.ts`
  - State: `mode: 'raw' | 'side-by-side' | 'formatted' | 'rendered'`
  - Per-tab dirty flag, auto-save interval management

- [x] **Step 13**: Create `frontend/src/stores/preferences.ts`
  - State mirrors `UserPreferences` struct
  - Actions: `load()` (calls `GET /api/preferences`), `save()` (calls `PUT /api/preferences`)

- [x] **Step 14**: Create `frontend/src/composables/useWebSocket.ts`
  - Connect/reconnect with exponential backoff (port logic from `AppState` in `app.ts`)
  - Parse incoming messages as typed `WsMessage`
  - Dispatch file change events into `files` and `tabs` stores

---

## Phase C — Vue Component Tree

> Steps 15–29 can be worked per-feature. Start from 15 downward.

- [x] **Step 15**: Create `frontend/src/main.ts`
  - Mount Vue app
  - Install Pinia, Vue Router, Vuetify (dark theme with Obsidian color tokens)
  - Import `@mdi/font/css/materialdesignicons.css`

- [x] **Step 16**: Create `frontend/src/App.vue`
  - Root `v-app` wrapper
  - Initialize WS connection on mount via `useWebSocket()`
  - Apply theme class

- [x] **Step 17**: Create `frontend/src/router/index.ts`
  - Route `/` → `MainLayout`
  - Route `/login` → `LoginPage` (guarded if auth enabled)
  - Navigation guard: redirect to `/login` if `auth.enabled && !auth.accessToken`

- [x] **Step 18**: Create `frontend/src/layouts/MainLayout.vue`
  - `v-navigation-drawer` (resizable sidebar)
  - `v-main` containing split pane area
  - Pane resize logic (draggable divider)

- [x] **Step 19**: Create `frontend/src/components/TopBar.vue`
  - `v-app-bar`
  - Vault selector (`v-select` bound to `vaults` store)
  - Search button (opens `SearchModal`)
  - Save status indicator
  - WebSocket connection indicator
  - Theme toggle, Plugin Manager button

- [x] **Step 20**: Create `frontend/src/components/sidebar/FileTree.vue`
  - Recursive `v-list` rendering `FileNode` tree
  - Right-click context menu: rename, delete, new file, new folder
  - Drag-and-drop reorder / move
  - Port tree-rendering and event logic from `app.ts`

- [x] **Step 21**: Create `frontend/src/components/sidebar/SidebarActions.vue`
  - Icon buttons: upload, new file, new folder, unique note, random note, daily note, insert template
  - Bulk selection toolbar (select all, deselect, download selected, delete selected)

- [x] **Step 22**: Create `frontend/src/components/tabs/TabBar.vue`
  - Tab strip per pane (`v-chip` or custom)
  - Dirty dot indicator
  - Close button (prompt if dirty)
  - Split/merge pane controls

- [x] **Step 23**: Create `frontend/src/components/editor/EditorPane.vue`
  - Dispatches to sub-components based on `tab.fileType` and `editor.mode`
  - Handles keyboard shortcuts (save, undo, redo, toggle mode)

- [x] **Step 24**: Create `frontend/src/components/editor/MarkdownEditor.vue`
  - Integrate `vendor/codejar` (keep existing CodeJar)
  - Use `composables/useUndoRedo.ts` (port `UndoRedoManager` from `frontend/src/editor/undo-redo.ts`)
  - Auto-save with debounce
  - Emit `change`, `save` events

- [x] **Step 25**: Create `frontend/src/components/editor/MarkdownPreview.vue`
  - Call `POST /api/vaults/{vault_id}/render` to render markdown server-side
  - Render HTML in `v-html` — use DOMPurify for XSS sanitization before binding
  - Handle wiki-link click navigation

- [x] **Step 26**: Create `frontend/src/components/editor/FrontmatterPanel.vue`
  - Expandable panel above editor
  - Key-value table editor for YAML frontmatter
  - Sync back to editor content on change

- [x] **Step 27**: Create viewer components:
  - `frontend/src/components/viewers/ImageViewer.vue`
  - `frontend/src/components/viewers/PdfViewer.vue` (integrate pdf.js)
  - `frontend/src/components/viewers/AudioVideoViewer.vue`

- [x] **Step 28**: Create modal components (all as `v-dialog`):
  - `components/modals/VaultManager.vue` — add/remove vaults
  - `components/modals/SearchModal.vue` — full-text search with result list
  - `components/modals/QuickSwitcher.vue` — fuzzy file switcher (Ctrl+P)
  - `components/modals/PluginManager.vue` — list/toggle plugins
  - `components/modals/TemplateSelector.vue`
  - `components/modals/ConflictResolver.vue` — diff view for write conflicts

- [x] **Step 29**: Port styles from the legacy `frontend/public/styles/main.css` into Vue/Vuetify component styles:
  - Map CSS custom properties (`--background-primary`, `--text-normal`, etc.) to Vuetify theme tokens
  - Keep Obsidian dark aesthetic
  - Use scoped `<style>` blocks in components for component-specific styles

---

## Phase D — Cargo Workspace Restructure

> Steps 30–33 are sequential.

> **Kickoff progress (2026-03-11):** Workspace is now rooted at `crates/obsidian-server`, `crates/obsidian-types`, and `crates/obsidian-client`. Server source has been moved to `crates/obsidian-server/src`, and `cargo build --workspace` compiles all members (existing server warnings remain). Core shared DTOs/enums from `models/mod.rs` have been extracted into `obsidian-types` and re-exported by the server models module.

- [x] **Step 30**: Convert root `Cargo.toml` to a workspace manifest:

  ```toml
  [workspace]
  members = [
      "crates/obsidian-server",
      "crates/obsidian-types",
      "crates/obsidian-client",
  ]
  resolver = "2"
  ```

- [x] **Step 31**: Create `crates/obsidian-types/`
  - Extract all `#[derive(Serialize, Deserialize)]` structs from `src/models/mod.rs`:
    `Vault`, `FileNode`, `FileContent`, `SearchResult`, `SearchMatch`,
    `FileChangeEvent`, `FileChangeType`, `UserPreferences`, `CreateVaultRequest`, etc.
  - Add new `WsMessage` enum (see Phase F Step 43)
  - Deps: only `serde`, `chrono`, `uuid` — keep it minimal, `no_std`-compatible where possible

- [x] **Step 32**: Move `src/` → `crates/obsidian-server/src/`
  - Update `Cargo.toml` for the new crate name and path
  - Add `obsidian-types = { path = "../obsidian-types" }` as dependency
  - Replace duplicated struct definitions with imports from `obsidian-types`
  - Keep `rust-embed` assets pointing at `../../target/frontend/` (relative to new crate root)

- [x] **Step 33**: Create `crates/obsidian-client/`
  - Typed `ObsidianClient` struct
  - Deps: `reqwest` (with TLS features), `tokio-tungstenite`, `serde_json`, `obsidian-types`
  - Methods (one per API endpoint): `list_vaults()`, `create_vault()`, `get_file_tree()`,
    `read_file()`, `write_file()`, `create_file()`, `delete_file()`, `rename_file()`,
    `search()`, `get_random_note()`, `get_daily_note()`, `get_preferences()`, `update_preferences()`,
    `get_recent_files()`, `record_recent_file()`, `render_markdown()`, `list_plugins()`, `toggle_plugin()`
  - Auth: `client.login(user, pass)` stores access token internally (auto-refresh moved to Phase E Step 39)
  - WS: `client.connect_ws() -> WsStream` for real-time events

---

## Phase E — JWT Authentication

> Depends on Phase D. Step 39 also updates `obsidian-client`.

- [x] **Step 34**: Add to `crates/obsidian-server/Cargo.toml`:
  - `jsonwebtoken = "9"`
  - `argon2 = "0.5"` (password hashing)
  - `rand = "0.9"` (already in root Cargo.toml, confirm available)

- [x] **Step 35**: Add SQLite migration for `users` table:

  ```sql
  CREATE TABLE IF NOT EXISTS users (
      id TEXT PRIMARY KEY,
      username TEXT NOT NULL UNIQUE,
      password_hash TEXT NOT NULL,
      created_at TEXT NOT NULL
  );
  ```

  Add a bootstrap mechanism to create first admin user from `config.toml` on startup if table is empty.

- [x] **Step 36**: Create `src/routes/auth.rs`:
  - `POST /api/auth/login` — validate credentials against `argon2` hash, return:

    ```json
    { "access_token": "...", "refresh_token": "...", "expires_in": 3600 }
    ```

  - `POST /api/auth/refresh` — validate refresh token, issue new access token
  - `POST /api/auth/logout` — invalidate refresh token (store revoked tokens in DB or use short-lived tokens only)

- [x] **Step 37**: Create `src/middleware/auth.rs`:
  - Actix-web middleware that validates `Authorization: Bearer <jwt>` header
  - Skip if `config.auth.enabled == false`
  - Skip paths: `/` (SPA root), `/api/auth/*`, static asset file extensions
  - On valid token: inject `UserId` into `req.extensions_mut()`
  - On invalid token: return `401 Unauthorized` JSON

- [x] **Step 38**: Add to `config.toml`:

  ```toml
  [auth]
  enabled = false          # set true to require login
  jwt_secret = ""          # auto-generated if empty, warn if default
  access_token_ttl = 3600  # seconds
  refresh_token_ttl = 604800
  ```

- [x] **Step 39**: Update `obsidian-client` crate:
  - `login(username, password)` stores access + refresh tokens
  - All request methods attach `Authorization` header
  - Token auto-refresh: check `expires_at` before each request, refresh if within 60 seconds

---

## Phase F — Desktop Sync API Enhancements

> Can be developed in parallel with Phase E after Phase D is complete.

- [x] **Step 40**: ETags for file endpoints:
  - `GET /api/vaults/{id}/files/{path}` — add `ETag` (hex of `sha256(content)` or mtime)
  - Support `If-None-Match` request header → `304 Not Modified` if ETag matches
  - This allows the desktop client to efficiently poll without re-downloading unchanged files

- [x] **Step 41**: Change log endpoint:
  - Add `file_change_log` SQLite table:

    ```sql
    CREATE TABLE file_change_log (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        vault_id TEXT NOT NULL,
        path TEXT NOT NULL,
        event_type TEXT NOT NULL,  -- 'created' | 'modified' | 'deleted' | 'renamed'
        etag TEXT,
        old_path TEXT,             -- for renames
        timestamp INTEGER NOT NULL -- unix milliseconds
    );
    ```

  - Insert on every `write_file`, `create_file`, `delete_file`, `rename_file`
  - Expose: `GET /api/vaults/{vault_id}/changes?since=<unix_ms>` → `FileChangeEvent[]`
  - Trim log entries older than configurable retention period (default 7 days)

- [x] **Step 42**: Batch sync check endpoint:
  - `POST /api/vaults/{vault_id}/sync`
  - Request body: `{ "files": [{ "path": "...", "client_etag": "...", "client_mtime": 12345 }] }`
  - Response: `{ "stale": ["path1", ...], "deleted": ["path2", ...], "server_newer": [...] }`
  - Used by desktop client on startup or reconnect to reconcile state in one round trip

- [x] **Step 43**: Formalized `WsMessage` enum in `obsidian-types`:

  ```rust
  #[derive(Serialize, Deserialize, Clone, Debug)]
  #[serde(tag = "type")]
  pub enum WsMessage {
      FileChanged { vault_id: String, path: String, event_type: FileChangeType, etag: Option<String>, timestamp: i64 },
      SyncPing,
      SyncPong { server_time: i64 },
      Error { message: String },
  }
  ```

  - Update `src/routes/ws.rs` to serialize events as `WsMessage` (replaces raw JSON)
  - Update frontend `useWebSocket.ts` to parse typed `WsMessage`

- [x] **Step 44**: CORS middleware:
  - Add `actix-cors` to `crates/obsidian-server/Cargo.toml`
  - Configure in `main.rs` with allowed origins from `config.toml`:

    ```toml
    [cors]
    allowed_origins = ["http://localhost:5173"]  # Vite dev server
    ```

- [x] **Step 45**: File metadata endpoint:
  - `GET /api/vaults/{vault_id}/files/{path}/metadata`
  - Response: `{ "path": "...", "size": 1234, "mtime": 12345, "etag": "...", "frontmatter_keys": ["title", "tags"] }`
  - Returns file info without body — used by desktop client for sync pre-checks

---

## Verification Checklist

- [x] `cd frontend && npm run build` — Vite builds to `target/frontend/` without errors
- [x] `cargo build --release` — rust-embed picks up Vite output, binary compiles clean
- [x] Open browser → full app loads from embedded assets; vault CRUD, file editor, search all functional
- [x] `cd frontend && npx vitest run` — store/composable unit tests pass
- [x] `POST /api/auth/login` returns JWT; protected routes reject `401` without token; `/` serves SPA
- [x] `GET /api/vaults/{id}/changes?since=0` returns populated change log entries
- [x] Connect WebSocket, edit file → receive `{ "type": "FileChanged", "etag": "...", ... }` message
- [x] `cargo build -p obsidian-client` compiles the client crate standalone
- [x] `cargo build -p obsidian-types` compiles with no server or client deps

---

## Further Considerations

1. **CodeJar vs. CodeMirror 6**: CodeJar is kept for initial port to reduce scope. CodeMirror 6 would give syntax highlighting keybindings and better mobile support — deferred.
2. **Plugin system in Vue**: Plugins currently inject raw HTML/JS. A defined Vue component slot API or iframe sandbox is needed — deferred.
3. **egui vs. iced**: Both are Rust-native; egui is simpler to prototype, iced is more declarative. The `obsidian-client` crate is framework-agnostic — decision deferred.
4. **Desktop crate location**: The future `crates/obsidian-desktop/` crate depends on both `obsidian-types` and `obsidian-client`. Adding it to the workspace later requires no changes to existing crates.
5. **Test migration**: Existing integration tests in `tests/` use relative import paths. After moving server code to `crates/obsidian-server/`, update test harness paths — minimal changes expected.
6. **Sync conflicts**: The current `ConflictResolver` uses a "last write wins with user prompt" strategy. For the desktop client, the `/changes` endpoint enables more sophisticated 3-way merge in the future.

---

## File Reference Map

| File | Phase | Change |
|------|-------|--------|
| `frontend/package.json` | A | Replace with Vite/Vue deps |
| `frontend/vite.config.ts` | A | Create new |
| `frontend/tsconfig.json` | A | Update for Vite |
| `frontend/src/app.ts` | B/C | Source of truth for port — do not delete until Phase C complete |
| `frontend/src/App.vue` + `frontend/src/**/*.vue` styles | C | Vuetify theme tokens + component-scoped styling |
| `src/assets.rs` | A | Update `#[folder]` path |
| `src/main.rs` | A, E | Update static path; register auth middleware |
| `src/models/mod.rs` | D | Extract types to `obsidian-types` crate |
| `src/routes/` (all) | D, E | Move to `crates/obsidian-server/src/routes/`; add `auth.rs` |
| `Cargo.toml` | D | Convert to workspace |
| `config.toml` | E, F | Add `[auth]` and `[cors]` sections |
