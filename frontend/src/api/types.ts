// Types that mirror the Rust backend models exactly.
// Keep these in sync with src/models/mod.rs

export interface Vault {
    id: string;
    name: string;
    path: string;
    path_exists: boolean;
    created_at: string;
    updated_at: string;
}

export interface CreateVaultRequest {
    name: string;
    path?: string;
}

export interface FileNode {
    name: string;
    path: string;
    is_directory: boolean;
    children?: FileNode[];
    size?: number;
    modified?: string;
}

export interface FileContent {
    path: string;
    content: string;
    modified: string;
    frontmatter?: Record<string, unknown>;
}

export interface UpdateFileRequest {
    content: string;
    last_modified?: string;
    frontmatter?: Record<string, unknown>;
}

export interface CreateFileRequest {
    path: string;
    content?: string;
}

export interface SearchMatch {
    line_number: number;
    line_text: string;
    match_start: number;
    match_end: number;
}

export interface SearchResult {
    path: string;
    title: string;
    matches: SearchMatch[];
    score: number;
}

export interface PagedSearchResult {
    results: SearchResult[];
    total_count: number;
    page: number;
    page_size: number;
}

export type FileChangeType =
    | 'created'
    | 'modified'
    | 'deleted'
    | { renamed: { from: string; to: string } };

export interface FileChangeEvent {
    vault_id: string;
    path: string;
    event_type: FileChangeType;
    timestamp: string;
}

export type EditorMode = 'raw' | 'side_by_side' | 'formatted_raw' | 'fully_rendered';

export interface UserPreferences {
    theme: string;
    editor_mode: EditorMode;
    font_size: number;
    window_layout?: string;
}

// Upload session types
export interface CreateUploadSessionRequest {
    filename: string;
    path: string;
    total_size?: number;
}

export interface UploadSessionResponse {
    session_id: string;
    uploaded_bytes: number;
    total_size?: number;
}

// Auth types (Phase E — used now so stores can be wired consistently)
export interface LoginRequest {
    username: string;
    password: string;
}

export interface LoginResponse {
    access_token: string;
    refresh_token: string;
    expires_in: number; // seconds
}

export type VaultRole = 'owner' | 'editor' | 'viewer';

export interface GroupInfo {
    id: string;
    name: string;
    created_at: string;
}

export interface GroupMember {
    user_id: string;
    username: string;
}

export interface AuthenticatedUserProfile {
    id: string;
    username: string;
    is_admin: boolean;
    must_change_password: boolean;
    groups: GroupInfo[];
    auth_method: string;
}

export interface AdminUser {
    id: string;
    username: string;
    is_admin: boolean;
    must_change_password: boolean;
    created_at: string;
}

export interface CreateUserRequest {
    username: string;
    temporary_password?: string;
    is_admin?: boolean;
}

export interface CreateUserResponse {
    id: string;
    username: string;
    temporary_password: string;
    is_admin: boolean;
    must_change_password: boolean;
}

export interface ChangePasswordRequest {
    current_password: string;
    new_password: string;
}

export interface CreateGroupRequest {
    name: string;
}

export interface AddGroupMemberRequest {
    user_id?: string;
    username?: string;
}

export interface ShareVaultWithUserRequest {
    user_id?: string;
    username?: string;
    role: VaultRole;
}

export interface ShareVaultWithGroupRequest {
    group_id: string;
    role: VaultRole;
}

export interface VaultShareEntry {
    principal_type: string;
    principal_id: string;
    principal_name: string;
    role: VaultRole;
}

export interface VaultShareList {
    owner_user_id?: string;
    user_shares: VaultShareEntry[];
    group_shares: VaultShareEntry[];
}

// WebSocket message envelope (Phase F formal type, used here for frontend)
export type WsMessage =
    | { type: 'FileChanged'; vault_id: string; path: string; event_type: FileChangeType; etag?: string; timestamp: number }
    | { type: 'SyncPing' }
    | { type: 'SyncPong'; server_time: number }
    | { type: 'Error'; message: string };

// UI-only tab type
export type FileType = 'markdown' | 'image' | 'pdf' | 'text' | 'audio' | 'video' | 'other';

export interface Tab {
    id: string;
    filePath: string;
    fileName: string;
    content: string;
    modified: string;
    isDirty: boolean;
    paneId: string;
    fileType: FileType;
    frontmatter?: Record<string, unknown>;
}

export interface Pane {
    id: string;
    flex: number;
    activeTabId: string | null;
}
