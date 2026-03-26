# Plan: Desktop Feature Parity, Sync, and Multi-User Support

## Current State Summary

- **Server**: Actix-web 4.9 with SQLite, full REST API, WebSocket file events, JWT auth, role-based vault access
- **Web Frontend**: Vue 3 + Vuetify, rich editor (TipTap + CodeJar), split panes, sidebar panels, modals, admin panel
- **Desktop App**: Iced-based, basic file browsing/editing, login, WebSocket sync, session persistence
- **Client Library**: Rust reqwest + tokio-tungstenite, full API coverage, token management

---

## 1. Desktop App — Feature Parity with Web UI

### 1.1 Sidebar Panels (Critical)

- [ ] **Outline Panel** — Interactive heading navigation with jump-to-heading, hierarchical indentation (web: `OutlinePanel.vue`)
- [ ] **Backlinks Panel** — Display files linking to the current note, click to open (web: `BacklinksPanel.vue`)
- [ ] **Outgoing Links Panel** — Show wiki links and markdown links from current file, resolve/open on click (web: `OutgoingLinksPanel.vue`)
- [ ] **Neighboring Files Panel** — Show previous/next markdown file in tree order (web: `NeighboringFilesPanel.vue`)
- [ ] **Bookmarks Panel** — Add/remove bookmarks with persistent storage, per-vault (web: `BookmarksPanel.vue`)
- [ ] **Recent Files Panel** — Dedicated panel showing last 20 accessed files (web: `RecentFilesPanel.vue`)
- [ ] **Tags Panel** — Tag list sorted by frequency, click to search by tag (web: `TagsPanel.vue`)
- [ ] **ML Insights Panel** — Generate outlines, organization suggestions with confidence scores, dry-run, undo (web: `MlInsightsPanel.vue`)

### 1.2 Editor & Editing (Critical)

- [ ] **Split Pane Editor** — Multiple panes with draggable splitter, vertical/horizontal orientation, per-pane tabs (web: `PaneContainer.vue`)
- [ ] **Tab System** — Multi-tab support with per-pane tab management, tab content caching (web: `tabs.ts` store)
- [ ] **Frontmatter Panel** — Dedicated YAML frontmatter editor with field-by-field editing (web: `FrontmatterPanel.vue`)
- [ ] **Rich Text Editor (TipTap equivalent)** — Enhanced markdown editing beyond plain text widget (web: `TiptapEditor.vue`)
- [ ] **Editor Toolbar** — Visual formatting buttons: Bold, Italic, Heading, List, Code, Link (web: `EditorToolbar.vue`)
- [ ] **Markdown Preview** — Rendered HTML preview with wiki link resolution and proper CSS (web: `MarkdownPreview.vue`)
- [ ] **Word Count Status Bar** — Real-time word/character count at bottom of editor (web: `EditorPane.vue`)
- [ ] **Auto-save with Debouncing** — 2-second debounced auto-save per tab (web: `EditorPane.vue`)
- [ ] **All 4 Editor Modes** — Ensure `plain_raw`, `side_by_side`, `formatted_raw`, `fully_rendered` all work

### 1.3 File Viewers (Critical)

- [ ] **Image Viewer** — Display PNG, JPG, GIF, WEBP, SVG with zoom/pan (web: `ImageViewer.vue`)
- [ ] **PDF Viewer** — Render and navigate PDF documents (web: `PdfViewer.vue`)
- [ ] **Audio/Video Player** — Playback for MP4, WebM, MP3, OGG, WAV, FLAC, etc. (web: `AudioVideoViewer.vue`)

### 1.4 Modals & Dialogs (Important)

- [ ] **Conflict Resolver Dialog** — Side-by-side conflict view, choose "yours" or "server" version (web: `ConflictResolver.vue`)
- [ ] **Template Selector** — Browse templates folder, create defaults if missing, insert into editor (web: `TemplateSelector.vue`)
- [ ] **Import Vault Dialog** — Drag-and-drop import, archive extraction (ZIP/TAR), progress tracking, conflict strategy (web: `ImportVaultDialog.vue`)
- [ ] **Plugin Manager** — List plugins, enable/disable toggles, descriptions (web: `PluginManager.vue`)
- [ ] **Vault Manager** — Create/select/delete vaults plus sharing with users/groups and role assignment (web: `VaultManager.vue`)
- [ ] **Search Modal** — Full-text search with tag support (#tag), result context, pagination (web: `SearchModal.vue`)
- [ ] **Quick Switcher** — Keyboard-driven file switcher (Cmd+P), arrow key navigation (web: `QuickSwitcher.vue`)

### 1.5 File Tree & Navigation (Important)

- [ ] **Drag-and-Drop File Reorganization** — Drag files between folders with automatic tree refresh (web: `FileTree.vue`)
- [ ] **File Tree Sorting** — Sort A-Z / Z-A toggle controls (web: `SidebarActions.vue`)
- [ ] **Sidebar Resize Handle** — Draggable sidebar width with persistent preference (web: `MainLayout.vue`)

### 1.6 Status & Indicators (Moderate)

- [ ] **WebSocket Connection Status** — Visual indicator (Connected/Offline) in top bar (web: `TopBar.vue`)
- [ ] **Unsaved Changes Indicator** — Count of unsaved tabs with color-coded warning (web: `TopBar.vue`)
- [ ] **Theme Toggle Button** — Light/dark theme switch in top bar (web: `TopBar.vue`)

### 1.7 User Profile & Admin (Moderate)

- [ ] **User Profile Menu** — Change password, admin panel access, sign out (web: `TopBar.vue`)
- [ ] **Admin Panel** — User management, group management (web: admin components)

---

## 2. Sync Capability Between Desktop and Server

### 2.1 Fix Existing Sync Issues (Critical)

- [ ] **Rename Event Semantics** — Server file watcher currently loses rename info (fires as delete+create); capture `old_path` in `FileChangeEvent` and propagate through WebSocket
- [ ] **ETag-Based Conflict Detection** — Implement optimistic locking: server returns ETag on read, client sends `If-Match` on write, server rejects stale writes with 409
- [ ] **Server-Side Conflict Detection** — When a write conflicts, return both versions so the client can show the conflict resolver dialog
- [ ] **WebSocket Heartbeat** — Implement `SyncPing`/`SyncPong` messages (already defined in `WsMessage` enum but unused) for connection keep-alive

### 2.2 Delta Sync (Important)

- [ ] **Expose Change Log API** — Create `GET /api/vaults/{id}/changes?since={timestamp}` endpoint from existing `file_change_log` table
- [ ] **Client Catch-Up on Reconnect** — When WebSocket reconnects after disconnect, fetch missed changes via change log API instead of full reload
- [ ] **Incremental File Sync** — Send diffs instead of full file content for large files (consider operational transforms or CRDT approach)

### 2.3 Offline Support for Desktop (Important)

- [ ] **Local Cache Layer** — Cache vault file tree and recently opened files on disk for offline access
- [ ] **Offline Edit Queue** — Queue edits made while offline, replay on reconnect with conflict detection
- [ ] **Sync Status Per File** — Track sync state: synced, pending upload, pending download, conflict
- [ ] **Background Sync Service** — Desktop background task that periodically syncs without blocking UI

### 2.4 Desktop Session & Auth Persistence (Important)

- [ ] **Persist Auth Tokens** — Save refresh token to OS keychain (keyring crate) so user stays logged in across restarts
- [ ] **Remember Me Feature** — Optional persistent login with secure token storage
- [ ] **Auto-Reconnect with Token Refresh** — On WebSocket disconnect, refresh token if expired before reconnecting

### 2.5 Advanced Sync Features (Future)

- [ ] **Selective Sync** — Allow users to choose which vaults/folders to sync locally
- [ ] **Bandwidth Throttling** — Configurable sync speed limits
- [ ] **Sync Pause/Resume** — Manual control over sync activity
- [ ] **Multi-Device Awareness** — Show which devices have a file open (presence indicators)

---

## 3. Multi-User Support with Username/Password on Server

### 3.1 Strengthen Existing Auth (Critical)

- [ ] **User Deletion Endpoint** — `DELETE /api/admin/users/{id}` with cascade handling (reassign/delete owned vaults, remove from groups)
- [ ] **User Deactivation** — Add `is_active` field to users table, prevent login when deactivated without deleting data
- [ ] **Failed Login Tracking** — Record failed login attempts, implement account lockout after N failures (e.g., 5 attempts, 15-minute lockout)
- [ ] **Password Policy Enforcement** — Configurable requirements: minimum length, complexity (uppercase, lowercase, digit, special char), expiration, reuse prevention
- [ ] **Audit Logging** — Log auth events (login, logout, failed login, password change, permission changes) to a dedicated `audit_log` table

### 3.2 Session Management (Important)

- [ ] **Server-Side Session Table** — Track active sessions (user_id, token_id, device_info, ip_address, created_at, last_active)
- [ ] **Explicit Token Revocation** — `POST /api/auth/revoke` to invalidate specific tokens server-side
- [ ] **Revoke All Sessions** — `POST /api/auth/revoke-all` to log out from all devices
- [ ] **Active Sessions View** — `GET /api/auth/sessions` to list all active sessions for current user
- [ ] **Concurrent Session Limits** — Configurable maximum simultaneous sessions per user

### 3.3 User Management UI (Important)

- [ ] **Admin User List** — Table with username, role, active status, last login, created date
- [ ] **Create User Form** — Username, temporary password, admin toggle, vault assignments
- [ ] **Edit User** — Change admin status, reset password, deactivate/reactivate
- [ ] **Delete User** — With confirmation and cascade options (reassign vaults or delete)
- [ ] **Bulk User Import** — CSV/JSON import for multiple user creation

### 3.4 Group Management UI (Important)

- [ ] **Group List View** — Table with group name, member count, vault count
- [ ] **Create/Edit Group** — Name, add/remove members, assign vault roles
- [ ] **Group Vault Permissions** — Assign vaults to groups with role selection (owner/editor/viewer)

### 3.5 Vault Sharing Improvements (Important)

- [ ] **Share Dialog in Desktop App** — Port vault sharing UI from web to desktop
- [ ] **Invitation System** — Share via invite link with role and expiration
- [ ] **Transfer Ownership** — Allow vault owner to transfer ownership to another user
- [ ] **Public/Private Vault Toggle** — Make vaults publicly readable without authentication

### 3.6 Advanced Auth Features (Future)

- [ ] **Two-Factor Authentication (TOTP)** — Add 2FA enrollment, verification, and backup codes
- [ ] **OAuth2/OIDC Provider Support** — Allow login via Google, GitHub, etc.
- [ ] **LDAP/Active Directory Integration** — Enterprise directory authentication
- [ ] **API Keys** — Generate long-lived API keys for programmatic access (separate from JWT)
- [ ] **Per-User Rate Limiting** — Prevent abuse with configurable rate limits per user/role

---

## Priority Order

### Phase 1 — Foundation (Highest Priority) ✅
1. ~~Desktop: Tab system + split pane editor~~
2. ~~Desktop: All sidebar panels (outline, backlinks, tags, bookmarks, recent)~~
3. ~~Sync: ETag-based conflict detection + conflict resolver~~
4. ~~Auth: User deletion/deactivation, failed login tracking, audit logging~~

### Phase 2 — Feature Completeness ✅
5. ~~Desktop: File viewers (image, PDF, audio/video)~~
6. ~~Desktop: All modals (search, quick switcher, vault manager, plugin manager)~~
7. ~~Sync: Change log API + client catch-up on reconnect~~
8. ~~Auth: Session management, password policy, admin UI improvements~~

### Phase 3 — Desktop Offline & Polish ✅
9. ~~Desktop: Frontmatter editor, editor toolbar, rich markdown preview~~
10. ~~Desktop: File tree sorting, collapsible sidebar sections~~
11. ~~Sync: Offline support with local cache + edit queue~~
12. ~~Desktop: Auth token persistence + auto-login~~

### Phase 4a — Quick Wins & Foundation ✅
13. ~~Auth: API key system (generate/revoke, middleware acceptance alongside JWT)~~
14. ~~Server: Per-user/IP rate limiting middleware (configurable in config.toml)~~
15. ~~Desktop: Theme toggle (wire preferences theme to Iced Theme enum)~~
16. ~~Server: Vault ownership transfer endpoint~~
17. ~~Server: Public/private vault toggle (anonymous read access for public vaults)~~

### Phase 4b — Security & Collaboration ✅
18. ~~Auth: TOTP 2FA (enrollment, QR code, verification on login, backup codes)~~
19. ~~Auth: Invitation system (invite links with role + expiration, acceptance endpoint)~~
20. ~~Server: Bulk user import (CSV/JSON endpoint for batch creation)~~

### Phase 4c — Admin UX & Sync ✅
21. ~~Desktop: Admin panel (user list, create/edit/deactivate/delete in Iced UI)~~
22. ~~Sync: Selective sync (config for which vaults/folders to sync, filter WS events)~~

### Phase 4d — Enterprise Auth ✅
23. ~~Auth: OAuth2/OIDC provider support (finish stubbed OIDC, Google/GitHub login)~~
24. ~~Auth: LDAP/Active Directory integration~~

---

## Remaining TODO — Deployment & Polish

### Deployment (before production use)
- [ ] Set a stable JWT secret in config (tokens break on restart without one)
- [ ] Set up TLS via reverse proxy (nginx/Caddy example in DEPLOYMENT.md)
- [ ] Wire `create_session` into `issue_tokens` so the sessions table is actually populated on login
- [ ] Allow anonymous read access for public vaults in auth middleware (visibility column exists, middleware doesn't check it yet)
- [ ] Desktop binary packaging (AppImage for Linux, DMG for macOS, MSI/installer for Windows)
- [ ] CI/CD pipeline (GitHub Actions: build, test, release binaries, Docker image push)

### Testing
- [ ] Frontend E2E tests (Playwright config exists, needs server fixture)
- [ ] Add integration tests for new Phase 1-4 endpoints (TOTP, invitations, API keys, audit log, bulk import)
- [ ] Load/stress testing for WebSocket sync under concurrent users
- [ ] Test OIDC flow end-to-end with a real provider (Google/GitHub)
- [ ] Test LDAP flow against a real directory (or dockerized OpenLDAP)

### Server Polish
- [ ] Clean up 10 compiler warnings (unused imports/variables)
- [ ] Add `GET /api/version` endpoint returning build version/git hash
- [ ] Add Prometheus `/metrics` endpoint for monitoring
- [ ] Graceful shutdown handling (drain WebSocket connections, flush change log)
- [ ] Request ID header for distributed tracing
- [ ] Per-user rate limiting (current limiter is per-IP only)
- [ ] Complete S3 storage backend (currently scaffolded but not functional)

### Desktop Polish
- [ ] Drag-and-drop file reorganization in file tree
- [ ] Sidebar resize handle (draggable divider between sidebar and editor)
- [ ] Collapsible toggles on all sidebar sections (currently only file tree)
- [ ] Keyboard shortcut help overlay (show all shortcuts in a modal)
- [ ] Multi-device presence indicators (show who else has a file open)

