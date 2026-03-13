import type {
    Vault,
    CreateVaultRequest,
    FileNode,
    FileContent,
    UpdateFileRequest,
    CreateFileRequest,
    PagedSearchResult,
    SearchResult,
    UserPreferences,
    UploadSessionResponse,
    LoginResponse,
    AuthenticatedUserProfile,
    GroupInfo,
    GroupMember,
    CreateGroupRequest,
    AddGroupMemberRequest,
    VaultShareList,
    ShareVaultWithUserRequest,
    ShareVaultWithGroupRequest,
    AdminUser,
    CreateUserRequest,
    CreateUserResponse,
    ChangePasswordRequest,
} from './types';
import { useAuthStore } from '@/stores/auth';

export class ApiError extends Error {
    constructor(
        public status: number,
        message: string,
        public body?: unknown,
    ) {
        super(message);
        this.name = 'ApiError';
    }
}

async function request<T>(
    url: string,
    options: RequestInit = {},
): Promise<T> {
    // Attach JWT header when a token is available.
    // Lazy-import to avoid Pinia not-yet-initialized errors during module load.
    let authHeader: Record<string, string> = {};
    try {
        const auth = useAuthStore();
        const token = auth.accessToken;
        if (token) {
            authHeader = { Authorization: `Bearer ${token}` };
        }
    } catch {
        // Pinia not initialized yet (SSR guard or early boot) — skip auth header.
    }

    const response = await fetch(url, {
        ...options,
        headers: {
            'Content-Type': 'application/json',
            ...authHeader,
            ...(options.headers ?? {}),
        },
    });

    if (!response.ok) {
        let body: unknown;
        try { body = await response.json(); } catch { /* empty */ }
        const message = (body as { error?: string })?.error ?? `HTTP ${response.status}`;
        throw new ApiError(response.status, message, body);
    }

    // 204 No Content
    if (response.status === 204) return undefined as unknown as T;

    return response.json() as Promise<T>;
}

// ── Vaults ───────────────────────────────────────────────────────────────────

export const apiListVaults = (): Promise<Vault[]> =>
    request('/api/vaults');

export const apiCreateVault = (data: CreateVaultRequest): Promise<Vault> =>
    request('/api/vaults', { method: 'POST', body: JSON.stringify(data) });

export const apiGetVault = (id: string): Promise<Vault> =>
    request(`/api/vaults/${id}`);

export const apiDeleteVault = (id: string): Promise<void> =>
    request(`/api/vaults/${id}`, { method: 'DELETE' });

// ── Files ─────────────────────────────────────────────────────────────────────

export const apiGetFileTree = (vaultId: string): Promise<FileNode[]> =>
    request(`/api/vaults/${vaultId}/files`);

export const apiReadFile = (vaultId: string, filePath: string): Promise<FileContent> =>
    request(`/api/vaults/${vaultId}/files/${filePath}`);

export const apiWriteFile = (
    vaultId: string,
    filePath: string,
    data: UpdateFileRequest,
): Promise<FileContent> =>
    request(`/api/vaults/${vaultId}/files/${filePath}`, {
        method: 'PUT',
        body: JSON.stringify(data),
    });

export const apiCreateFile = (
    vaultId: string,
    data: CreateFileRequest,
): Promise<FileContent> =>
    request(`/api/vaults/${vaultId}/files`, {
        method: 'POST',
        body: JSON.stringify(data),
    });

export const apiDeleteFile = (vaultId: string, filePath: string): Promise<void> =>
    request(`/api/vaults/${vaultId}/files/${filePath}`, { method: 'DELETE' });

export const apiCreateDirectory = (vaultId: string, path: string): Promise<void> =>
    request(`/api/vaults/${vaultId}/directories`, {
        method: 'POST',
        body: JSON.stringify({ path }),
    });

export const apiRenameFile = (
    vaultId: string,
    from: string,
    to: string,
    strategy: 'fail' | 'overwrite' | 'rename' = 'fail',
): Promise<{ new_path: string }> =>
    request(`/api/vaults/${vaultId}/rename`, {
        method: 'POST',
        body: JSON.stringify({ from, to, strategy }),
    });

// ── Raw / Assets ─────────────────────────────────────────────────────────────

export const apiRawFileUrl = (vaultId: string, filePath: string): string =>
    `/api/vaults/${vaultId}/raw/${filePath}`;

export const apiThumbnailUrl = (
    vaultId: string,
    filePath: string,
    width = 200,
    height = 200,
): string =>
    `/api/vaults/${vaultId}/thumbnail/${filePath}?width=${width}&height=${height}`;

// ── Search ────────────────────────────────────────────────────────────────────

export const apiSearch = (
    vaultId: string,
    query: string,
    page = 1,
    pageSize = 50,
): Promise<PagedSearchResult> =>
    request(
        `/api/vaults/${vaultId}/search?q=${encodeURIComponent(query)}&page=${page}&page_size=${pageSize}`,
    );

export const apiReindex = (vaultId: string): Promise<{ indexed_files: number }> =>
    request(`/api/vaults/${vaultId}/reindex`, { method: 'POST' });

// ── Markdown ──────────────────────────────────────────────────────────────────

export const apiRenderMarkdown = (content: string): Promise<string> =>
    request<string>('/api/render', {
        method: 'POST',
        body: JSON.stringify({ content }),
    });

export const apiRenderMarkdownInVault = (
    vaultId: string,
    content: string,
    currentFile?: string,
): Promise<string> =>
    request<string>(`/api/vaults/${vaultId}/render`, {
        method: 'POST',
        body: JSON.stringify({ content, current_file: currentFile }),
    });

// ── Resolve wiki link ─────────────────────────────────────────────────────────

export const apiResolveWikiLink = (
    vaultId: string,
    link: string,
    currentFile?: string,
): Promise<{ path: string; exists: boolean; ambiguous: boolean; alternatives: string[] }> =>
    request(`/api/vaults/${vaultId}/resolve-link`, {
        method: 'POST',
        body: JSON.stringify({ link, current_file: currentFile }),
    });

// ── Special notes ─────────────────────────────────────────────────────────────

export const apiGetRandomNote = (vaultId: string): Promise<{ path: string }> =>
    request(`/api/vaults/${vaultId}/random`);

export const apiGetDailyNote = (
    vaultId: string,
    date: string,
): Promise<FileContent> =>
    request(`/api/vaults/${vaultId}/daily`, {
        method: 'POST',
        body: JSON.stringify({ date }),
    });

// ── Preferences ───────────────────────────────────────────────────────────────

export const apiGetPreferences = (): Promise<UserPreferences> =>
    request('/api/preferences');

export const apiUpdatePreferences = (prefs: UserPreferences): Promise<UserPreferences> =>
    request('/api/preferences', { method: 'PUT', body: JSON.stringify(prefs) });

export const apiResetPreferences = (): Promise<UserPreferences> =>
    request('/api/preferences/reset', { method: 'POST' });

// ── Recent files ──────────────────────────────────────────────────────────────

export const apiGetRecentFiles = (vaultId: string): Promise<string[]> =>
    request(`/api/vaults/${vaultId}/recent`);

export const apiRecordRecentFile = (vaultId: string, path: string): Promise<void> => {
    // Fire-and-forget
    void request(`/api/vaults/${vaultId}/recent`, {
        method: 'POST',
        body: JSON.stringify({ path }),
    });
    return Promise.resolve();
};

// ── Upload ────────────────────────────────────────────────────────────────────

export const apiCreateUploadSession = (
    vaultId: string,
    filename: string,
    totalSize: number,
    path = '',
): Promise<UploadSessionResponse> =>
    request(`/api/vaults/${vaultId}/upload-sessions`, {
        method: 'POST',
        body: JSON.stringify({ filename, total_size: totalSize, path }),
    });

export const apiUploadChunk = (
    vaultId: string,
    sessionId: string,
    chunk: Blob,
): Promise<{ uploaded_bytes: number }> =>
    fetch(`/api/vaults/${vaultId}/upload-sessions/${sessionId}`, {
        method: 'PUT',
        body: chunk,
    }).then((r) => r.json());

export const apiFinishUploadSession = (
    vaultId: string,
    sessionId: string,
    filename: string,
    path = '',
): Promise<unknown> =>
    request(`/api/vaults/${vaultId}/upload-sessions/${sessionId}/finish`, {
        method: 'POST',
        body: JSON.stringify({ filename, path }),
    });

export const apiDownloadFileUrl = (vaultId: string, filePath: string): string =>
    `/api/vaults/${vaultId}/download/${filePath}`;

export const apiDownloadZip = (vaultId: string, paths: string[]): Promise<Blob> =>
    fetch(`/api/vaults/${vaultId}/download-zip`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ paths }),
    }).then((r) => {
        if (!r.ok) throw new ApiError(r.status, 'Failed to download zip');
        return r.blob();
    });

// ── Plugins ───────────────────────────────────────────────────────────────────

export const apiListPlugins = (): Promise<{ plugins: unknown[] }> =>
    request('/api/plugins');

export const apiTogglePlugin = (
    pluginId: string,
    enabled: boolean,
): Promise<{ success: boolean; plugin_id: string; enabled: boolean }> =>
    request(`/api/plugins/${pluginId}/toggle`, {
        method: 'POST',
        body: JSON.stringify({ enabled }),
    });

// ── Auth (Phase E — endpoints may not yet exist; gracefully no-op) ────────────

export const apiLogin = (username: string, password: string): Promise<LoginResponse> =>
    request('/api/auth/login', {
        method: 'POST',
        body: JSON.stringify({ username, password }),
    });

export const apiRefreshToken = (refreshToken: string): Promise<LoginResponse> =>
    request('/api/auth/refresh', {
        method: 'POST',
        body: JSON.stringify({ refresh_token: refreshToken }),
    });

export const apiLogout = (): Promise<void> =>
    request('/api/auth/logout', { method: 'POST' });

export const apiMe = (): Promise<AuthenticatedUserProfile> =>
    request('/api/auth/me');

export const apiChangePassword = (data: ChangePasswordRequest): Promise<{ success: boolean }> =>
    request('/api/auth/change-password', {
        method: 'POST',
        body: JSON.stringify(data),
    });

// ── Admin ────────────────────────────────────────────────────────────────────

export const apiListUsers = (): Promise<AdminUser[]> =>
    request('/api/admin/users');

export const apiCreateUser = (data: CreateUserRequest): Promise<CreateUserResponse> =>
    request('/api/admin/users', {
        method: 'POST',
        body: JSON.stringify(data),
    });

// ── Groups ───────────────────────────────────────────────────────────────────

export const apiListGroups = (): Promise<GroupInfo[]> =>
    request('/api/groups');

export const apiCreateGroup = (data: CreateGroupRequest): Promise<GroupInfo> =>
    request('/api/groups', {
        method: 'POST',
        body: JSON.stringify(data),
    });

export const apiListGroupMembers = (groupId: string): Promise<GroupMember[]> =>
    request(`/api/groups/${groupId}/members`);

export const apiAddGroupMember = (
    groupId: string,
    data: AddGroupMemberRequest,
): Promise<GroupMember[]> =>
    request(`/api/groups/${groupId}/members`, {
        method: 'POST',
        body: JSON.stringify(data),
    });

export const apiRemoveGroupMember = (groupId: string, userId: string): Promise<void> =>
    request(`/api/groups/${groupId}/members/${userId}`, {
        method: 'DELETE',
    });

// ── Vault sharing ────────────────────────────────────────────────────────────

export const apiListVaultShares = (vaultId: string): Promise<VaultShareList> =>
    request(`/api/vaults/${vaultId}/shares`);

export const apiShareVaultWithUser = (
    vaultId: string,
    data: ShareVaultWithUserRequest,
): Promise<VaultShareList> =>
    request(`/api/vaults/${vaultId}/shares/users`, {
        method: 'POST',
        body: JSON.stringify(data),
    });

export const apiShareVaultWithGroup = (
    vaultId: string,
    data: ShareVaultWithGroupRequest,
): Promise<VaultShareList> =>
    request(`/api/vaults/${vaultId}/shares/groups`, {
        method: 'POST',
        body: JSON.stringify(data),
    });

export const apiRevokeVaultUserShare = (
    vaultId: string,
    userId: string,
): Promise<VaultShareList> =>
    request(`/api/vaults/${vaultId}/shares/users/${userId}`, {
        method: 'DELETE',
    });

export const apiRevokeVaultGroupShare = (
    vaultId: string,
    groupId: string,
): Promise<VaultShareList> =>
    request(`/api/vaults/${vaultId}/shares/groups/${groupId}`, {
        method: 'DELETE',
    });
