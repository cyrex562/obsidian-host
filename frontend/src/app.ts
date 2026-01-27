// @ts-ignore
import { CodeJar } from './vendor/codejar/codejar.js';

declare const htmx: any;
declare const pdfjsLib: any;
declare const hljs: any;

// Types
interface Vault {
    id: string;
    name: string;
    path: string;
    path_exists?: boolean;
    created_at: string;
    updated_at: string;
}

interface FileNode {
    name: string;
    path: string;
    is_directory: boolean;
    children?: FileNode[];
    size?: number;
    modified?: string;
}

interface FileContent {
    path: string;
    content: string;
    modified: string;
    frontmatter?: any;
}

interface SearchResult {
    path: string;
    title: string;
    matches: SearchMatch[];
    score: number;
}

interface SearchMatch {
    line_number: number;
    line_text: string;
    match_start: number;
    match_end: number;
}

interface Tab {
    id: string;
    filePath: string;
    fileName: string;
    content: string;
    modified: string;
    isDirty: boolean;
    pane: string;
    fileType: 'markdown' | 'image' | 'pdf' | 'text' | 'audio' | 'video' | 'other';
    frontmatter?: any;
    autoSaveInterval?: number;
    undoManager?: UndoRedoManager;
}

// Undo/Redo System - Command Pattern Implementation

interface EditCommand {
    execute(): string;  // Returns new content
    undo(): string;     // Returns previous content
    timestamp: number;
}

class TextChangeCommand implements EditCommand {
    timestamp: number;

    constructor(
        private oldContent: string,
        private newContent: string
    ) {
        this.timestamp = Date.now();
    }

    execute(): string {
        return this.newContent;
    }

    undo(): string {
        return this.oldContent;
    }
}

class UndoRedoManager {
    private undoStack: EditCommand[] = [];
    private redoStack: EditCommand[] = [];
    private maxStackSize: number = 100;
    private lastContent: string;
    private debounceTimeout: number | null = null;
    private pendingOldContent: string | null = null;
    private debounceMs: number = 300;

    constructor(initialContent: string) {
        this.lastContent = initialContent;
    }

    // Call this when content changes - handles debouncing for smoother UX
    recordChange(newContent: string): void {
        if (newContent === this.lastContent) return;

        // Store the old content before any pending changes
        if (this.pendingOldContent === null) {
            this.pendingOldContent = this.lastContent;
        }

        // Clear any existing debounce timer
        if (this.debounceTimeout !== null) {
            clearTimeout(this.debounceTimeout);
        }

        // Set a new debounce timer
        this.debounceTimeout = window.setTimeout(() => {
            this.commitChange(newContent);
        }, this.debounceMs);

        // Update lastContent immediately for tracking
        this.lastContent = newContent;
    }

    // Force commit any pending changes (e.g., before save or undo)
    flushPendingChanges(): void {
        if (this.debounceTimeout !== null) {
            clearTimeout(this.debounceTimeout);
            this.debounceTimeout = null;
        }
        if (this.pendingOldContent !== null && this.pendingOldContent !== this.lastContent) {
            const command = new TextChangeCommand(this.pendingOldContent, this.lastContent);
            this.pushCommand(command);
        }
        this.pendingOldContent = null;
    }

    private commitChange(newContent: string): void {
        if (this.pendingOldContent === null || this.pendingOldContent === newContent) {
            this.pendingOldContent = null;
            return;
        }

        const command = new TextChangeCommand(this.pendingOldContent, newContent);
        this.pushCommand(command);
        this.pendingOldContent = null;
        this.debounceTimeout = null;
    }

    private pushCommand(command: EditCommand): void {
        this.undoStack.push(command);
        this.redoStack = []; // Clear redo stack on new change

        // Limit stack size
        if (this.undoStack.length > this.maxStackSize) {
            this.undoStack.shift();
        }
    }

    undo(): string | null {
        this.flushPendingChanges();

        if (this.undoStack.length === 0) return null;

        const command = this.undoStack.pop()!;
        this.redoStack.push(command);

        const content = command.undo();
        this.lastContent = content;
        return content;
    }

    redo(): string | null {
        this.flushPendingChanges();

        if (this.redoStack.length === 0) return null;

        const command = this.redoStack.pop()!;
        this.undoStack.push(command);

        const content = command.execute();
        this.lastContent = content;
        return content;
    }

    canUndo(): boolean {
        return this.undoStack.length > 0 || this.pendingOldContent !== null;
    }

    canRedo(): boolean {
        return this.redoStack.length > 0;
    }

    // Reset the manager (e.g., after save or reload)
    reset(content: string): void {
        this.undoStack = [];
        this.redoStack = [];
        this.lastContent = content;
        this.pendingOldContent = null;
        if (this.debounceTimeout !== null) {
            clearTimeout(this.debounceTimeout);
            this.debounceTimeout = null;
        }
    }

    // Get current content (for sync purposes)
    getCurrentContent(): string {
        return this.lastContent;
    }
}

// File type detection helpers
function getFileType(filePath: string): 'markdown' | 'image' | 'pdf' | 'text' | 'audio' | 'video' | 'other' {
    const ext = filePath.split('.').pop()?.toLowerCase();
    if (!ext) return 'other';

    if (ext === 'md') return 'markdown';
    if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) return 'image';
    if (ext === 'pdf') return 'pdf';
    if (['mp3', 'wav', 'ogg'].includes(ext)) return 'audio';
    if (['mp4', 'webm'].includes(ext)) return 'video';
    if (['txt', 'json', 'js', 'ts', 'css', 'html', 'xml', 'rs', 'py', 'java', 'c', 'cpp', 'h', 'go', 'yaml', 'yml', 'toml', 'ini', 'sh', 'bat', 'mdx'].includes(ext)) return 'text';
    return 'other';
}

function isImageFile(filePath: string): boolean {
    return getFileType(filePath) === 'image';
}

// Utility
function debounce(func: Function, wait: number) {
    let timeout: any;
    return function (this: any, ...args: any[]) {
        clearTimeout(timeout);
        timeout = setTimeout(() => func.apply(this, args), wait);
    };
}

// App State
export class AppState {
    currentVaultId: string | null = null;
    vaults: Vault[] = [];
    openTabs: Map<string, Tab> = new Map();
    activeTabId: string | null = null;
    private _editorMode: 'raw' | 'side-by-side' | 'formatted' | 'rendered' = 'raw';
    selectedFiles: Set<string> = new Set(); // For bulk operations

    // Split pane state
    panes: { id: string; flex: number; activeTabId: string | null; }[] = [];
    activePaneId: string = 'pane-1';
    splitOrientation: 'vertical' | 'horizontal' = 'vertical';  // vertical = side-by-side, horizontal = stacked

    constructor() {
        // Defaults will be overridden by loadPreferences()
        this.panes.push({ id: 'pane-1', flex: 1, activeTabId: null });
    }

    get editorMode() {
        return this._editorMode;
    }

    set editorMode(mode) {
        this._editorMode = mode;
    }
    ws: WebSocket | null = null;
    searchDebounce: number | null = null;
    quickSwitcherDebounce: number | null = null;
    recentFiles: string[] = [];
    wsReconnectAttempts: number = 0;
    wsReconnectTimeout: number | null = null;
    wsMaxReconnectDelay: number = 30000; // 30 seconds max
    conflictData: { filePath: string; yourVersion: string; serverVersion: string; } | null = null;

    setVault(vaultId: string | null) {
        this.currentVaultId = vaultId;
    }

    addRecentFile(filePath: string) {
        if (!this.currentVaultId) return;
        // Remove if exists to move to top
        this.recentFiles = this.recentFiles.filter(p => p !== filePath);
        // Add to front
        this.recentFiles.unshift(filePath);
        // Limit to 20
        if (this.recentFiles.length > 20) {
            this.recentFiles.pop();
        }
        // Saving is now handled by API call in UIManager
    }

    addTab(tab: Tab) {
        this.openTabs.set(tab.id, tab);
    }

    removeTab(tabId: string) {
        this.openTabs.delete(tabId);
    }

    getTab(tabId: string): Tab | undefined {
        return this.openTabs.get(tabId);
    }

    setActiveTab(tabId: string | null) {
        this.activeTabId = tabId;
    }
}

// API Client
export class ApiClient {
    private baseUrl = '';

    async getVaults(): Promise<Vault[]> {
        const response = await fetch(`${this.baseUrl}/api/vaults`);
        if (!response.ok) throw new Error('Failed to fetch vaults');
        return response.json();
    }

    async createVault(name: string, path: string): Promise<Vault> {
        const response = await fetch(`${this.baseUrl}/api/vaults`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, path }),
        });
        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to create vault');
        }
        return response.json();
    }

    async deleteVault(vaultId: string): Promise<void> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}`, {
            method: 'DELETE',
        });
        if (!response.ok) throw new Error('Failed to delete vault');
    }

    async getFileTree(vaultId: string): Promise<FileNode[]> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/files`);
        if (!response.ok) throw new Error('Failed to fetch file tree');
        return response.json();
    }

    async readFile(vaultId: string, filePath: string): Promise<FileContent> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/files/${filePath}`);
        if (!response.ok) throw new Error('Failed to read file');
        return response.json();
    }

    async writeFile(vaultId: string, filePath: string, content: string, lastModified?: string, frontmatter?: any): Promise<FileContent> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/files/${filePath}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                content,
                last_modified: lastModified,
                frontmatter: frontmatter
            }),
        });
        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to write file');
        }
        return response.json();
    }

    async createFile(vaultId: string, filePath: string, content?: string): Promise<FileContent> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/files`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path: filePath, content }),
        });
        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to create file');
        }
        return response.json();
    }

    async deleteFile(vaultId: string, filePath: string): Promise<void> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/files/${filePath}`, {
            method: 'DELETE',
        });
        if (!response.ok) throw new Error('Failed to delete file');

    }

    async createDirectory(vaultId: string, path: string): Promise<void> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/directories`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path }),
        });
        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to create directory');
        }
    }

    async renameFile(vaultId: string, from: string, to: string, strategy: string = 'fail'): Promise<{ new_path: string }> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/rename`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ from, to, strategy }),
        });
        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to rename file');
        }
        return response.json();
    }

    async search(vaultId: string, query: string, limit: number = 50): Promise<SearchResult[]> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/search?q=${encodeURIComponent(query)}&page_size=${limit}`);
        if (!response.ok) throw new Error('Failed to search');
        const pagedResult = await response.json();
        return pagedResult.results || [];
    }

    async getRandomNote(vaultId: string): Promise<{ path: string }> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/random`);
        if (!response.ok) throw new Error('Failed to get random note');
        return response.json();
    }

    async getDailyNote(vaultId: string, date: string): Promise<FileContent> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/daily`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ date }),
        });
        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to get daily note');
        }
        return response.json();
    }

    async uploadFiles(vaultId: string, files: FileList, targetPath: string = '', onProgress?: (loaded: number, total: number) => void): Promise<any> {
        // If single large file (> 5MB), use chunked upload
        if (files.length === 1 && files[0].size > 5 * 1024 * 1024) {
            return this.uploadFileChunked(vaultId, files[0], targetPath, onProgress);
        }

        const formData = new FormData();

        // Add all files to form data
        for (let i = 0; i < files.length; i++) {
            formData.append('file', files[i]);
        }

        // Add target path if specified
        if (targetPath) {
            formData.append('path', targetPath);
        }

        return new Promise((resolve, reject) => {
            const xhr = new XMLHttpRequest();

            // Track upload progress
            xhr.upload.addEventListener('progress', (e) => {
                if (e.lengthComputable && onProgress) {
                    onProgress(e.loaded, e.total);
                }
            });

            xhr.addEventListener('load', () => {
                if (xhr.status >= 200 && xhr.status < 300) {
                    resolve(JSON.parse(xhr.responseText));
                } else {
                    reject(new Error(`Upload failed: ${xhr.statusText}`));
                }
            });

            xhr.addEventListener('error', () => {
                reject(new Error('Upload failed'));
            });

            xhr.open('POST', `${this.baseUrl}/api/vaults/${vaultId}/upload`);
            xhr.send(formData);
        });
    }

    // Chunked Upload Methods
    async createUploadSession(vaultId: string, filename: string, totalSize: number, path: string = ''): Promise<{ session_id: string, uploaded_bytes: number }> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/upload-sessions`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ filename, total_size: totalSize, path }),
        });
        if (!response.ok) throw new Error('Failed to create upload session');
        return response.json();
    }

    async uploadChunk(vaultId: string, sessionId: string, chunk: Blob): Promise<{ uploaded_bytes: number }> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/upload-sessions/${sessionId}`, {
            method: 'PUT',
            body: chunk,
        });
        if (!response.ok) throw new Error('Failed to upload chunk');
        return response.json();
    }

    async getUploadSessionStatus(vaultId: string, sessionId: string): Promise<{ session_id: string, uploaded_bytes: number }> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/upload-sessions/${sessionId}`);
        if (!response.ok) throw new Error('Failed to get upload status');
        return response.json();
    }

    async finishUploadSession(vaultId: string, sessionId: string, filename: string, path: string = ''): Promise<any> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/upload-sessions/${sessionId}/finish`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ filename, path }),
        });
        if (!response.ok) throw new Error('Failed to finish upload');
        return response.json();
    }

    async uploadFileChunked(vaultId: string, file: File, targetPath: string = '', onProgress?: (loaded: number, total: number) => void): Promise<any> {
        const CHUNK_SIZE = 1024 * 1024 * 2; // 2MB chunks
        const totalSize = file.size;

        // Resume key based on file characteristics
        const resumeKey = `upload_session_${vaultId}_${file.name}_${file.size}_${file.lastModified}`;
        let sessionId = localStorage.getItem(resumeKey);
        let uploadedBytes = 0;

        try {
            if (sessionId) {
                // Check status
                try {
                    const status = await this.getUploadSessionStatus(vaultId, sessionId);
                    uploadedBytes = status.uploaded_bytes;
                    console.log(`Resuming upload for ${file.name} from ${uploadedBytes} bytes`);
                } catch (e) {
                    // Session invalid, start over
                    console.warn('Invalid session, starting over');
                    sessionId = null;
                    uploadedBytes = 0;
                }
            }

            if (!sessionId) {
                const session = await this.createUploadSession(vaultId, file.name, totalSize, targetPath);
                sessionId = session.session_id;
                localStorage.setItem(resumeKey, sessionId);
            }
        } catch (e) {
            throw e;
        }

        if (!sessionId) throw new Error("Could not initialize upload session");

        // Upload loop
        while (uploadedBytes < totalSize) {
            const end = Math.min(uploadedBytes + CHUNK_SIZE, totalSize);
            const chunk = file.slice(uploadedBytes, end);

            try {
                await this.uploadChunk(vaultId, sessionId, chunk);
                uploadedBytes = end;

                if (onProgress) {
                    onProgress(uploadedBytes, totalSize);
                }
            } catch (e) {
                // If chunk fails, we throw. Client can invoke upload again to resume.
                console.error("Chunk upload failed", e);
                // We don't remove the key so it can be resumed
                throw new Error(`Upload failed at ${Math.round(uploadedBytes / 1024 / 1024)}MB. Try again to resume.`);
            }
        }

        // Finish
        const result = await this.finishUploadSession(vaultId, sessionId, file.name, targetPath);
        localStorage.removeItem(resumeKey);
        return { uploaded: [result] };
    }

    async downloadFile(vaultId: string, filePath: string): Promise<void> {
        const url = `${this.baseUrl}/api/vaults/${vaultId}/download/${filePath}`;
        const link = document.createElement('a');
        link.href = url;
        link.download = filePath.split('/').pop() || 'download';
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
    }

    async downloadZip(vaultId: string, paths: string[]): Promise<void> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/download-zip`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ paths }),
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to download files');
        }

        // Get the blob and trigger download
        const blob = await response.blob();
        const url = window.URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = url;

        // Extract filename from Content-Disposition header
        const contentDisposition = response.headers.get('Content-Disposition');
        let filename = 'download.zip';
        if (contentDisposition) {
            const filenameMatch = contentDisposition.match(/filename="(.+)"/);
            if (filenameMatch) {
                filename = filenameMatch[1];
            }
        }

        link.download = filename;
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
        window.URL.revokeObjectURL(url);
    }

    async renderMarkdown(content: string): Promise<string> {
        const response = await fetch(`${this.baseUrl}/api/render`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ content }),
        });
        if (!response.ok) throw new Error('Failed to render markdown');
        return response.text();
    }

    async resolveWikiLink(vaultId: string, link: string, currentFile?: string): Promise<{ path: string; exists: boolean; ambiguous: boolean; alternatives: string[] }> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/resolve-link`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ link, current_file: currentFile }),
        });
        if (!response.ok) throw new Error('Failed to resolve wiki link');
        return response.json();
    }

    // Preferences
    async getPreferences(): Promise<UserPreferences> {
        const response = await fetch(`${this.baseUrl}/api/preferences`);
        if (!response.ok) throw new Error('Failed to get preferences');
        return response.json();
    }

    async updatePreferences(prefs: UserPreferences): Promise<void> {
        const response = await fetch(`${this.baseUrl}/api/preferences`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(prefs),
        });
        if (!response.ok) throw new Error('Failed to update preferences');
        return;
    }

    async resetPreferences(): Promise<void> {
        const response = await fetch(`${this.baseUrl}/api/preferences/reset`, {
            method: 'POST',
        });
        if (!response.ok) throw new Error('Failed to reset preferences');
        return;
    }

    // Recent Files
    async getRecentFiles(vaultId: string): Promise<string[]> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/recent`);
        if (!response.ok) {
            console.warn('Failed to get recent files, returning empty list');
            return [];
        }
        return response.json();
    }

    async recordRecentFile(vaultId: string, filePath: string): Promise<void> {
        // Fire and forget, but log error if fails
        fetch(`${this.baseUrl}/api/vaults/${vaultId}/recent`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path: filePath }),
        }).catch(e => console.error('Failed to record recent file:', e));
    }
}

interface UserPreferences {
    theme: string;
    editor_mode: string;
    font_size: number;
    window_layout?: string;
}

// UI Manager
export class UIManager {
    private currentJar: any = null;
    private currentQuill: any = null;
    private paneJars: Map<string, any> = new Map();
    private paneQuills: Map<string, any> = new Map();
    private pdfStates: Map<string, any> = new Map();

    constructor(private state: AppState, private api: ApiClient) { }

    async loadVaults() {
        try {
            this.state.vaults = await this.api.getVaults();
            this.renderVaultSelector();
            this.renderVaultManager();
        } catch (error) {
            console.error('Failed to load vaults:', error);
            alert('Failed to load vaults: ' + error);
        }
    }

    renderVaultSelector() {
        const select = document.getElementById('vault-select') as HTMLSelectElement;
        if (!select) return;

        select.innerHTML = '<option value="">Select a vault...</option>';
        for (const vault of this.state.vaults) {
            const option = document.createElement('option');
            option.value = vault.id;
            const missing = vault.path_exists === false;
            option.textContent = missing ? `${vault.name} (missing)` : vault.name;
            if (missing) {
                option.style.color = '#6b7280';
                option.style.fontStyle = 'italic';
            }
            if (vault.id === this.state.currentVaultId) {
                option.selected = true;
            }
            select.appendChild(option);
        }
    }

    renderVaultManager() {
        const list = document.getElementById('vault-list');
        if (!list) return;

        list.innerHTML = '';
        if (this.state.vaults.length === 0) {
            const empty = document.createElement('div');
            empty.className = 'vault-item vault-missing';
            empty.textContent = 'No vaults yet. Add one below.';
            list.appendChild(empty);
            return;
        }

        for (const vault of this.state.vaults) {
            const missing = vault.path_exists === false;
            const item = document.createElement('div');
            item.className = 'vault-item';

            const meta = document.createElement('div');
            meta.className = 'vault-meta';

            const name = document.createElement('div');
            name.className = `vault-name${missing ? ' vault-missing' : ''}`;
            name.textContent = vault.name;

            const path = document.createElement('div');
            path.className = `vault-path${missing ? ' vault-missing' : ''}`;
            path.textContent = vault.path;

            const status = document.createElement('div');
            status.className = `vault-status${missing ? ' vault-missing' : ''}`;
            status.textContent = missing ? 'Missing (path not found)' : 'Available';

            meta.appendChild(name);
            meta.appendChild(path);
            meta.appendChild(status);

            const actions = document.createElement('div');
            actions.className = 'vault-actions';

            const removeBtn = document.createElement('button');
            removeBtn.className = 'btn btn-secondary';
            removeBtn.textContent = 'Remove';
            removeBtn.setAttribute('data-action', 'delete-vault');
            removeBtn.setAttribute('data-vault-id', vault.id);

            actions.appendChild(removeBtn);

            item.appendChild(meta);
            item.appendChild(actions);

            list.appendChild(item);
        }
    }

    async switchVault(vaultId: string) {
        if (!vaultId) return;

        const vault = this.state.vaults.find(v => v.id === vaultId);
        if (vault && vault.path_exists === false) {
            alert('This vault path no longer exists. Please remove it or choose another vault.');
            const select = document.getElementById('vault-select') as HTMLSelectElement | null;
            if (select) {
                select.value = this.state.currentVaultId || '';
            }
            return;
        }

        this.state.setVault(vaultId);
        this.loadRecentFiles(vaultId);
        this.closeAllTabs();

        // HTMX Integration for File Tree
        const fileTree = document.getElementById('file-tree');
        if (fileTree && typeof htmx !== 'undefined') {
            fileTree.setAttribute('hx-get', `/api/vaults/${vaultId}/files-html`);
            fileTree.setAttribute('hx-trigger', 'load');
            htmx.process(fileTree);
            // Trigger load immediately
            htmx.trigger(fileTree, 'load');
        } else {
            await this.loadFileTree();
        }
    }

    async loadFileTree() {
        if (!this.state.currentVaultId) return;

        try {
            const tree = await this.api.getFileTree(this.state.currentVaultId);
            this.renderFileTree(tree);
        } catch (error) {
            console.error('Failed to load file tree:', error);
            alert('Failed to load file tree: ' + error);
        }
    }

    renderFileTree(nodes: FileNode[], parentElement?: HTMLElement) {
        const container = parentElement || document.getElementById('file-tree');
        if (!container) return;

        if (!parentElement) {
            container.innerHTML = '';

            // Show "No files found" message for empty vault
            if (!nodes || nodes.length === 0) {
                const emptyMessage = document.createElement('p');
                emptyMessage.textContent = 'No files found';
                emptyMessage.style.padding = '1rem';
                emptyMessage.style.textAlign = 'center';
                emptyMessage.style.color = 'var(--text-muted)';
                container.appendChild(emptyMessage);
                return;
            }
        }

        for (const node of nodes) {
            // Create wrapper for folder nodes to contain item + children
            const nodeWrapper = document.createElement('div');
            nodeWrapper.className = 'file-tree-node';

            const item = document.createElement('div');
            item.className = 'file-tree-item' + (node.is_directory ? ' folder' : '');

            // Determine icon
            let icon = node.is_directory ? '📁' : '📄';
            if (!node.is_directory) {
                const ext = node.name.split('.').pop()?.toLowerCase();
                if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext || '')) icon = '🖼️';
                else if (ext === 'pdf') icon = '📕';
                else if (['mp3', 'wav', 'ogg'].includes(ext || '')) icon = '🎵';
                else if (['mp4', 'webm'].includes(ext || '')) icon = '🎬';
                else if (['js', 'ts', 'py', 'rs', 'html', 'css', 'json'].includes(ext || '')) icon = '📝';
                else if (ext === 'md') icon = '📝';
            }

            // Size text
            let metaText = '';
            if (node.size !== undefined) {
                if (node.size < 1024) metaText = `${node.size} B`;
                else if (node.size < 1024 * 1024) metaText = `${(node.size / 1024).toFixed(1)} KB`;
                else metaText = `${(node.size / (1024 * 1024)).toFixed(1)} MB`;
            }

            // Add checkbox for files (not directories)
            const isSelected = this.state.selectedFiles.has(node.path);
            const checkboxHtml = !node.is_directory
                ? `<input type="checkbox" class="file-checkbox" data-path="${node.path}" ${isSelected ? 'checked' : ''}>`
                : '';

            // Add expand/collapse arrow for folders
            const arrowHtml = node.is_directory && node.children && node.children.length > 0
                ? `<span class="folder-arrow">▶</span>`
                : '';

            item.innerHTML = `
                ${arrowHtml}
                ${checkboxHtml}
                <span class="icon">${icon}</span>
                <span class="name">${node.name}</span>
                ${!node.is_directory ? `<span class="meta" style="font-size: 0.8em; color: var(--text-muted); margin-left: auto;">${metaText}</span>` : ''}
            `;

            if (isSelected) {
                item.classList.add('selected');
            }

            // Checkbox event handler
            const checkbox = item.querySelector('.file-checkbox') as HTMLInputElement;
            if (checkbox) {
                checkbox.addEventListener('change', (e) => {
                    e.stopPropagation();
                    if (checkbox.checked) {
                        this.state.selectedFiles.add(node.path);
                        item.classList.add('selected');
                    } else {
                        this.state.selectedFiles.delete(node.path);
                        item.classList.remove('selected');
                    }
                    this.updateBulkActionsToolbar();
                });
            }

            // Hover preview for images
            if (!node.is_directory && ['png', 'jpg', 'jpeg', 'gif', 'webp'].includes(node.name.split('.').pop()?.toLowerCase() || '')) {
                const preview = document.createElement('div');
                preview.className = 'hover-preview hidden';
                preview.style.position = 'fixed';
                preview.style.zIndex = '1000';
                preview.style.background = 'var(--bg-secondary)';
                preview.style.border = '1px solid var(--border-color)';
                preview.style.padding = '5px';
                preview.style.borderRadius = '4px';
                preview.style.pointerEvents = 'none';
                preview.innerHTML = 'Loading preview...';
                document.body.appendChild(preview);

                item.addEventListener('mouseenter', (e) => {
                    if (!this.state.currentVaultId) return;

                    const rect = item.getBoundingClientRect();
                    preview.style.left = `${rect.right + 10}px`;
                    preview.style.top = `${rect.top}px`;
                    preview.classList.remove('hidden');

                    // Load thumbnail
                    // We can use the thumbnail endpoint we created earlier!
                    // /api/vaults/{vault_id}/thumbnail/{file_path}?width=200&height=200
                    const thumbUrl = `/api/vaults/${this.state.currentVaultId}/thumbnail/${encodeURIComponent(node.path)}?width=200&height=200`;

                    preview.innerHTML = `<img src="${thumbUrl}" style="max-width: 200px; max-height: 200px; display: block;">`;
                });

                item.addEventListener('mouseleave', () => {
                    preview.classList.add('hidden');
                });

                // Cleanup on item removal (not perfect but acceptable for now)
                // Ideally we'd remove element from body when item is removed from DOM.
            }


            if (!node.is_directory) {
                item.tabIndex = 0; // Make focusable
                item.addEventListener('click', () => this.openFile(node.path));

                // Quick Look trigger
                item.addEventListener('keydown', (e) => {
                    if (e.code === 'Space') {
                        e.preventDefault();
                        this.showQuickLook(node);
                    }
                });
            }

            // Add context menu support
            item.addEventListener('contextmenu', (e) => {
                e.preventDefault();
                this.showFileContextMenu(e, node);
            });

            // Add drag and drop support for moving files
            item.draggable = true;
            item.setAttribute('data-path', node.path);
            item.setAttribute('data-is-directory', String(node.is_directory));

            item.addEventListener('dragstart', (e: DragEvent) => {
                if (!e.dataTransfer) return;
                e.dataTransfer.effectAllowed = 'move';
                e.dataTransfer.setData('text/plain', node.path);
                e.dataTransfer.setData('application/obsidian-file', JSON.stringify({
                    path: node.path,
                    isDirectory: node.is_directory
                }));
                item.classList.add('dragging');
            });

            item.addEventListener('dragend', () => {
                item.classList.remove('dragging');
                // Remove all drag-over highlights
                document.querySelectorAll('.file-tree-item.drag-over').forEach(el => {
                    el.classList.remove('drag-over');
                });
            });

            // Allow dropping on folders
            if (node.is_directory) {
                item.addEventListener('dragover', (e: DragEvent) => {
                    e.preventDefault();
                    if (!e.dataTransfer) return;

                    // Check if this is a file from the file tree or from OS
                    const hasObsidianFile = e.dataTransfer.types.includes('application/obsidian-file');
                    const hasFiles = e.dataTransfer.types.includes('Files');

                    if (hasObsidianFile || hasFiles) {
                        e.dataTransfer.dropEffect = 'move';
                        item.classList.add('drag-over');
                    }
                });

                item.addEventListener('dragleave', (e: DragEvent) => {
                    // Only remove highlight if we're actually leaving the element
                    const rect = item.getBoundingClientRect();
                    const x = e.clientX;
                    const y = e.clientY;
                    if (x < rect.left || x >= rect.right || y < rect.top || y >= rect.bottom) {
                        item.classList.remove('drag-over');
                    }
                });

                item.addEventListener('drop', async (e: DragEvent) => {
                    e.preventDefault();
                    e.stopPropagation();
                    item.classList.remove('drag-over');

                    if (!e.dataTransfer || !this.state.currentVaultId) return;

                    // Check if dropping files from OS
                    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
                        await this.handleFileUploadDrop(e.dataTransfer.files, node.path);
                        return;
                    }

                    // Handle moving files within vault
                    const obsidianData = e.dataTransfer.getData('application/obsidian-file');
                    if (obsidianData) {
                        const draggedFile = JSON.parse(obsidianData);
                        const sourcePath = draggedFile.path;

                        // Don't allow dropping on itself or its children
                        if (sourcePath === node.path || sourcePath.startsWith(node.path + '/')) {
                            return;
                        }

                        // Calculate new path
                        const fileName = sourcePath.split('/').pop() || '';
                        const newPath = node.path ? `${node.path}/${fileName}` : fileName;

                        try {
                            await this.api.renameFile(this.state.currentVaultId, sourcePath, newPath);
                            await this.loadFileTree();

                            // Update tabs if it's an open file
                            if (!draggedFile.isDirectory) {
                                const oldTabId = `${this.state.currentVaultId}:${sourcePath}`;
                                const tab = this.state.getTab(oldTabId);
                                if (tab) {
                                    this.state.removeTab(oldTabId);
                                    await this.openFile(newPath);
                                }
                            }
                        } catch (error) {
                            console.error('Failed to move file:', error);
                            alert('Failed to move file: ' + error);
                        }
                    }
                });
            }

            // Add expand/collapse for folders
            if (node.is_directory && node.children && node.children.length > 0) {
                const arrow = item.querySelector('.folder-arrow');
                if (arrow) {
                    arrow.addEventListener('click', (e) => {
                        e.stopPropagation();
                        nodeWrapper.classList.toggle('collapsed');
                        if (nodeWrapper.classList.contains('collapsed')) {
                            arrow.textContent = '▶';
                        } else {
                            arrow.textContent = '▼';
                        }
                    });
                    // Start expanded
                    arrow.textContent = '▼';
                }
            }

            nodeWrapper.appendChild(item);
            container.appendChild(nodeWrapper);

            if (node.is_directory && node.children && node.children.length > 0) {
                const childContainer = document.createElement('div');
                childContainer.className = 'file-tree-children';
                nodeWrapper.appendChild(childContainer);
                this.renderFileTree(node.children, childContainer);
            }
        }
    }

    async showQuickLook(node: FileNode) {
        if (!this.state.currentVaultId) return;

        const modal = document.getElementById('quick-look-modal');
        const contentContainer = document.getElementById('quick-look-content');
        const title = document.getElementById('quick-look-title');
        const meta = document.getElementById('quick-look-meta');
        const openBtn = document.getElementById('quick-look-open');

        if (!modal || !contentContainer || !title) return;

        title.textContent = node.name;
        if (meta) meta.textContent = node.size ? `${(node.size / 1024).toFixed(1)} KB` : '';

        // Setup Open Button
        const newOpenBtn = openBtn?.cloneNode(true);
        if (openBtn && newOpenBtn) {
            openBtn.parentNode?.replaceChild(newOpenBtn, openBtn);
            newOpenBtn.addEventListener('click', () => {
                this.openFile(node.path);
                this.hideModal('quick-look-modal');
            });
        }

        contentContainer.innerHTML = '<div class="loading">Loading preview...</div>';
        this.showModal('quick-look-modal');

        const fileType = getFileType(node.name);
        try {
            if (fileType === 'markdown' || fileType === 'text') {
                // Fetch content
                const fileContent = await this.api.readFile(this.state.currentVaultId, node.path);
                let contentHtml = '';

                if (fileType === 'markdown') {
                    contentHtml = `<div class="markdown-content">${await this.renderMarkdown(fileContent.content)}</div>`;
                } else {
                    contentHtml = `<pre style="padding: 20px; overflow: auto; height: 100%;"><code>${this.escapeHtml(fileContent.content)}</code></pre>`;
                }
                contentContainer.innerHTML = contentHtml;

                if (fileType === 'text') {
                    // Syntax highlight
                    const block = contentContainer.querySelector('code');
                    if (block) hljs.highlightElement(block);
                }

            } else if (fileType === 'image') {
                const src = `/api/vaults/${this.state.currentVaultId}/raw/${node.path}`;
                contentContainer.innerHTML = `
                    <div style="display: flex; justify-content: center; align-items: center; height: 100%; background: var(--bg-primary);">
                        <img src="${src}" style="max-width: 100%; max-height: 100%; object-fit: contain;">
                    </div>
                 `;
            } else if (fileType === 'pdf') {
                // Use PDF viewer logic - reuse pdfStates but with a unique ID for preview
                const src = `/api/vaults/${this.state.currentVaultId}/raw/${node.path}`;

                // Clean up previous preview state if exists
                this.pdfStates.delete('preview');

                // Mock a tab for the viewer
                const mockTab = {
                    id: 'preview',
                    content: src,
                    filePath: node.path,
                    //... other props not needed by viewer
                } as any;

                this.renderPdfViewer(contentContainer, mockTab);

            } else if (fileType === 'audio') {
                const src = `/api/vaults/${this.state.currentVaultId}/raw/${node.path}`;
                contentContainer.innerHTML = `
                    <div style="display: flex; justify-content: center; align-items: center; height: 100%;">
                        <audio controls src="${src}" style="width: 80%;"></audio>
                    </div>
                 `;
            } else if (fileType === 'video') {
                const src = `/api/vaults/${this.state.currentVaultId}/raw/${node.path}`;
                contentContainer.innerHTML = `
                    <div style="display: flex; justify-content: center; align-items: center; height: 100%;">
                        <video controls src="${src}" style="max-width: 100%; max-height: 100%;"></video>
                    </div>
                 `;
            } else {
                contentContainer.innerHTML = `
                    <div class="empty-state">
                        <p>Preview not available for this file type.</p>
                    </div>
                 `;
            }

        } catch (e) {
            contentContainer.innerHTML = `<div class="error" style="padding: 20px;">Failed to load preview: ${e}</div>`;
        }
    }

    showFileContextMenu(event: MouseEvent, node: FileNode) {
        // Remove any existing context menu
        const existingMenu = document.querySelector('.context-menu');
        existingMenu?.remove();

        const menu = document.createElement('div');
        menu.className = 'context-menu';
        menu.style.position = 'fixed';
        menu.style.left = `${event.clientX}px`;
        menu.style.top = `${event.clientY}px`;

        // New File option (only for directories)
        if (node.is_directory) {
            const newFileOption = document.createElement('div');
            newFileOption.className = 'context-menu-item';
            newFileOption.textContent = 'New File';
            newFileOption.setAttribute('data-action', 'new-file');
            newFileOption.addEventListener('click', async () => {
                const fileName = prompt('Enter file name:');
                if (!fileName || !this.state.currentVaultId) {
                    menu.remove();
                    return;
                }

                const filePath = node.path ? `${node.path}/${fileName}` : fileName;
                try {
                    await this.api.createFile(this.state.currentVaultId, filePath, '');
                    await this.loadFileTree();
                    await this.openFile(filePath);
                } catch (error) {
                    console.error('Failed to create file:', error);
                    alert('Failed to create file: ' + error);
                }
                menu.remove();
            });
            menu.appendChild(newFileOption);

            // New Folder option (only for directories)
            const newFolderOption = document.createElement('div');
            newFolderOption.className = 'context-menu-item';
            newFolderOption.textContent = 'New Folder';
            newFolderOption.setAttribute('data-action', 'new-folder');
            newFolderOption.addEventListener('click', async () => {
                const folderName = prompt('Enter folder name:');
                if (!folderName || !this.state.currentVaultId) {
                    menu.remove();
                    return;
                }

                const folderPath = node.path ? `${node.path}/${folderName}` : folderName;
                try {
                    await this.api.createDirectory(this.state.currentVaultId, folderPath);
                    await this.loadFileTree();
                } catch (error) {
                    console.error('Failed to create folder:', error);
                    alert('Failed to create folder: ' + error);
                }
                menu.remove();
            });
            menu.appendChild(newFolderOption);
        }

        // Rename option (for both files and directories)
        const renameOption = document.createElement('div');
        renameOption.className = 'context-menu-item';
        renameOption.textContent = 'Rename';
        renameOption.setAttribute('data-action', 'rename');
        renameOption.addEventListener('click', async () => {
            const currentName = node.path.split('/').pop() || '';
            const newName = prompt('Enter new name:', currentName);
            if (!newName || !this.state.currentVaultId || newName === currentName) {
                menu.remove();
                return;
            }

            const parentPath = node.path.split('/').slice(0, -1).join('/');
            const newPath = parentPath ? `${parentPath}/${newName}` : newName;

            try {
                await this.api.renameFile(this.state.currentVaultId, node.path, newPath);
                await this.loadFileTree();

                // If it's an open file, update the tab
                if (!node.is_directory) {
                    const oldTabId = `${this.state.currentVaultId}:${node.path}`;
                    const tab = this.state.getTab(oldTabId);
                    if (tab) {
                        this.state.removeTab(oldTabId);
                        await this.openFile(newPath);
                    }
                }
            } catch (error) {
                console.error('Failed to rename:', error);
                alert('Failed to rename: ' + error);
            }
            menu.remove();
        });
        menu.appendChild(renameOption);

        // Delete option (for both files and directories)
        const deleteOption = document.createElement('div');
        deleteOption.className = 'context-menu-item';
        deleteOption.textContent = 'Delete';
        deleteOption.setAttribute('data-action', 'delete');
        deleteOption.addEventListener('click', async () => {
            const itemType = node.is_directory ? 'folder' : 'file';
            if (!confirm(`Are you sure you want to delete this ${itemType}?`)) {
                menu.remove();
                return;
            }

            if (!this.state.currentVaultId) {
                menu.remove();
                return;
            }

            try {
                await this.api.deleteFile(this.state.currentVaultId, node.path);
                await this.loadFileTree();

                // If it's an open file, close the tab
                if (!node.is_directory) {
                    const tabId = `${this.state.currentVaultId}:${node.path}`;
                    if (this.state.getTab(tabId)) {
                        this.closeTab(tabId);
                    }
                }
            } catch (error) {
                console.error('Failed to delete:', error);
                alert('Failed to delete: ' + error);
            }
            menu.remove();
        });
        menu.appendChild(deleteOption);

        const downloadOption = document.createElement('div');
        downloadOption.className = 'context-menu-item';
        downloadOption.textContent = node.is_directory ? 'Download as ZIP' : 'Download';
        downloadOption.setAttribute('data-action', 'download');
        downloadOption.addEventListener('click', async () => {
            if (!this.state.currentVaultId) return;

            try {
                if (node.is_directory) {
                    await this.api.downloadZip(this.state.currentVaultId, [node.path]);
                } else {
                    await this.api.downloadFile(this.state.currentVaultId, node.path);
                }
            } catch (error) {
                console.error('Download failed:', error);
                alert('Failed to download: ' + error);
            }
            menu.remove();
        });

        menu.appendChild(downloadOption);
        document.body.appendChild(menu);

        // Close menu when clicking outside
        const closeMenu = (e: MouseEvent) => {
            if (!menu.contains(e.target as Node)) {
                menu.remove();
                document.removeEventListener('click', closeMenu);
            }
        };
        setTimeout(() => {
            document.addEventListener('click', closeMenu);
        }, 0);
    }

    async handleFileUploadDrop(files: FileList, targetPath: string = '') {
        if (!this.state.currentVaultId || !files || files.length === 0) return;

        try {
            const result = await this.api.uploadFiles(
                this.state.currentVaultId,
                files,
                targetPath
            );

            // Refresh file tree to show new files
            await this.loadFileTree();

            // Show success message
            const fileCount = files.length;
            const message = fileCount === 1
                ? `File "${files[0].name}" uploaded successfully`
                : `${fileCount} files uploaded successfully`;

            this.showToast(message, 'success');
        } catch (error) {
            console.error('Failed to upload files:', error);
            this.showToast('Failed to upload files: ' + error, 'error');
        }
    }

    showToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
        const toast = document.createElement('div');
        toast.className = `toast toast-${type}`;
        toast.textContent = message;
        toast.style.position = 'fixed';
        toast.style.bottom = '20px';
        toast.style.right = '20px';
        toast.style.padding = '12px 20px';
        toast.style.borderRadius = '4px';
        toast.style.zIndex = '10000';
        toast.style.boxShadow = '0 4px 6px rgba(0,0,0,0.1)';

        if (type === 'success') {
            toast.style.backgroundColor = '#4caf50';
            toast.style.color = 'white';
        } else if (type === 'error') {
            toast.style.backgroundColor = '#f44336';
            toast.style.color = 'white';
        } else {
            toast.style.backgroundColor = 'var(--bg-secondary)';
            toast.style.color = 'var(--text-primary)';
            toast.style.border = '1px solid var(--border-color)';
        }

        document.body.appendChild(toast);

        setTimeout(() => {
            toast.style.transition = 'opacity 0.3s';
            toast.style.opacity = '0';
            setTimeout(() => {
                document.body.removeChild(toast);
            }, 300);
        }, 3000);
    }

    async openFile(filePath: string) {
        if (!this.state.currentVaultId) return;

        // Add to recents
        this.state.addRecentFile(filePath);
        this.api.recordRecentFile(this.state.currentVaultId, filePath);

        const tabId = `${this.state.currentVaultId}:${filePath}`;
        const existingTab = this.state.getTab(tabId);

        if (existingTab) {
            this.activateTab(tabId);
            return;
        }

        const fileType = getFileType(filePath);

        try {
            let content = '';
            let modified = new Date().toISOString();
            let frontmatter = undefined;

            // For text-based files, read the content
            if (fileType === 'markdown' || fileType === 'text') {
                const fileContent = await this.api.readFile(this.state.currentVaultId, filePath);
                content = fileContent.content;
                modified = fileContent.modified;
                frontmatter = fileContent.frontmatter;
                modified = fileContent.modified;
                frontmatter = fileContent.frontmatter;
            } else {
                // For binary files and unsupported types, we'll use the raw endpoint directly
                content = `/api/vaults/${this.state.currentVaultId}/raw/${filePath}`;
            }

            const tab: Tab = {
                id: tabId,
                filePath: filePath,
                fileName: filePath.split('/').pop() || filePath,
                content: content,
                modified: modified,
                isDirty: false,
                pane: this.state.activePaneId,
                fileType: fileType,
                frontmatter: frontmatter,
                undoManager: new UndoRedoManager(content),
            };

            this.state.addTab(tab);
            this.renderTabs();
            this.activateTab(tabId);
        } catch (error) {
            console.error('Failed to open file:', error);
            alert('Failed to open file: ' + error);
        }
    }

    async saveFile(tabId: string) {
        const tab = this.state.getTab(tabId);
        if (!tab || !this.state.currentVaultId) return;

        const saveStatus = document.getElementById('save-status');
        if (saveStatus) {
            saveStatus.textContent = 'Saving...';
            saveStatus.className = 'save-status saving';
        }

        try {
            const updated = await this.api.writeFile(
                this.state.currentVaultId,
                tab.filePath,
                tab.content,
                tab.modified,
                tab.frontmatter
            );

            tab.modified = updated.modified;
            tab.isDirty = false;
            this.renderTabs();

            if (saveStatus) {
                saveStatus.textContent = 'Saved';
                saveStatus.className = 'save-status';
                setTimeout(() => {
                    if (saveStatus.textContent === 'Saved') {
                        saveStatus.textContent = '';
                    }
                }, 2000);
            }
        } catch (error) {
            console.error('Failed to save file:', error);
            if (saveStatus) {
                saveStatus.textContent = 'Save Failed';
                saveStatus.className = 'save-status error';
            }
            // Don't alert on auto-save errors to avoid disrupting user
            console.error('Auto-save failed: ' + error);
        }
    }

    renderTabs() {
        const tabsContainer = document.getElementById('tabs');
        if (!tabsContainer) return;

        tabsContainer.innerHTML = '';

        for (const [tabId, tab] of this.state.openTabs) {
            const tabElement = document.createElement('div');
            tabElement.className = 'tab' + (tabId === this.state.activeTabId ? ' active' : '');
            // Dynamic pane styling (optional, skipped for now)
            tabElement.innerHTML = `
                <span class="tab-name">${tab.isDirty ? '• ' : ''}${tab.fileName}</span>
                <button class="tab-close">✕</button>
            `;
            tabElement.setAttribute('data-tab-id', tabId);
            tabElement.draggable = true;

            tabElement.querySelector('.tab-name')?.addEventListener('click', () => this.activateTab(tabId));
            tabElement.querySelector('.tab-close')?.addEventListener('click', (e) => {
                e.stopPropagation();
                this.closeTab(tabId);
            });

            // Context menu for split view
            tabElement.addEventListener('contextmenu', (e) => {
                e.preventDefault();
                this.showTabContextMenu(e as MouseEvent, tabId);
            });

            // Drag events for split view
            tabElement.addEventListener('dragstart', (e) => {
                e.dataTransfer?.setData('text/plain', tabId);
                tabElement.classList.add('dragging');
            });

            tabElement.addEventListener('dragend', () => {
                tabElement.classList.remove('dragging');
            });

            tabsContainer.appendChild(tabElement);
        }

        // Setup drop zones on panes
        this.setupPaneDropZones();
    }

    showTabContextMenu(e: MouseEvent, tabId: string) {
        // Remove any existing context menu
        const existing = document.querySelector('.tab-context-menu');
        if (existing) existing.remove();

        const menu = document.createElement('div');
        menu.className = 'tab-context-menu';
        menu.style.position = 'fixed';
        menu.style.left = `${e.clientX}px`;
        menu.style.top = `${e.clientY}px`;
        menu.style.zIndex = '1000';

        const tab = this.state.getTab(tabId);
        // Heuristic: Is in the first pane?
        const isMainPane = this.state.panes.length > 0 && tab?.pane === this.state.panes[0].id;

        menu.innerHTML = `
            <div class="context-menu-item" data-action="toggle-pane">
                ${!isMainPane ? 'Move to Main Pane' : 'Split / New Pane'}
            </div>
            <div class="context-menu-item" data-action="close">Close Tab</div>
            <div class="context-menu-item" data-action="close-others">Close Other Tabs</div>
        `;

        document.body.appendChild(menu);

        // Handle menu item clicks
        menu.querySelectorAll('.context-menu-item').forEach(item => {
            item.addEventListener('click', () => {
                const action = item.getAttribute('data-action');
                if (action === 'toggle-pane') {
                    if (!isMainPane) {
                        this.moveTabToPane1(tabId);
                    } else {
                        this.openInSplitView(tabId);
                    }
                } else if (action === 'close') {
                    this.closeTab(tabId);
                } else if (action === 'close-others') {
                    this.closeOtherTabs(tabId);
                }
                menu.remove();
            });
        });

        // Close menu on click outside
        const closeMenu = (e: MouseEvent) => {
            if (!menu.contains(e.target as Node)) {
                menu.remove();
                document.removeEventListener('click', closeMenu);
            }
        };
        setTimeout(() => document.addEventListener('click', closeMenu), 0);
    }

    moveTabToPane1(tabId: string) {
        const tab = this.state.getTab(tabId);
        if (tab && this.state.panes.length > 0) {
            const mainPane = this.state.panes[0];
            tab.pane = mainPane.id;
            mainPane.activeTabId = tabId;
            this.state.activePaneId = mainPane.id;
            this.state.setActiveTab(tabId);
            this.renderEditor();
            this.renderTabs();
        }
    }



    closeOtherTabs(keepTabId: string) {
        const tabsToClose: string[] = [];
        for (const [tabId, tab] of this.state.openTabs) {
            if (tabId !== keepTabId) {
                if (tab.isDirty) {
                    if (!confirm(`${tab.fileName} has unsaved changes. Close anyway?`)) {
                        continue;
                    }
                }
                tabsToClose.push(tabId);
            }
        }

        for (const tabId of tabsToClose) {
            const tab = this.state.getTab(tabId);
            if (tab?.autoSaveInterval) {
                clearInterval(tab.autoSaveInterval);
            }
            this.state.removeTab(tabId);
        }

        this.state.setActiveTab(keepTabId);
        this.renderTabs();
        this.renderEditor();
    }

    setupPaneDropZones() {
        const splitContainer = document.getElementById('split-container');
        if (!splitContainer) return;

        const handleDragOver = (e: DragEvent) => {
            e.preventDefault();
            e.dataTransfer!.dropEffect = 'move';
        };

        const handleDrop = (e: DragEvent) => {
            e.preventDefault();
            const tabId = e.dataTransfer?.getData('text/plain');
            if (!tabId) return;

            const tab = this.state.getTab(tabId);
            if (!tab) return;

            const target = e.target as HTMLElement;
            const paneEl = target.closest('.editor-pane');
            if (!paneEl) return;
            const paneId = paneEl.id;

            const targetPane = this.state.panes.find(p => p.id === paneId);
            if (!targetPane) return;

            tab.pane = targetPane.id;
            targetPane.activeTabId = tab.id;
            this.state.activePaneId = targetPane.id;
            this.state.setActiveTab(tab.id);

            this.renderEditor();
            this.renderTabs();
        };

        splitContainer.addEventListener('dragover', handleDragOver);
        splitContainer.addEventListener('drop', handleDrop);
    }

    activateTab(tabId: string) {
        this.state.setActiveTab(tabId);

        const tab = this.state.getTab(tabId);
        if (tab) {
            // Ensure tab is assigned to a valid pane
            let pane = this.state.panes.find(p => p.id === tab.pane);
            if (!pane) {
                // Fallback to active pane
                pane = this.state.panes.find(p => p.id === this.state.activePaneId);
                if (pane) {
                    tab.pane = pane.id;
                }
            }

            if (pane) {
                pane.activeTabId = tabId;
                // Also update activePaneId to this pane
                this.state.activePaneId = pane.id;
            }
        }

        this.renderTabs();
        this.renderEditor();

        // Update properties panel if it's open
        const propertiesPanel = document.getElementById('properties-panel');
        if (propertiesPanel && !propertiesPanel.classList.contains('hidden')) {
            this.renderProperties();
        }

        this.savePreferences();
    }

    closeTab(tabId: string) {
        const tab = this.state.getTab(tabId);
        if (tab?.isDirty) {
            if (!confirm('File has unsaved changes. Close anyway?')) {
                return;
            }
        }

        if (tab?.autoSaveInterval) {
            clearInterval(tab.autoSaveInterval);
        }
        this.state.removeTab(tabId);

        if (this.state.activeTabId === tabId) {
            const tabs = Array.from(this.state.openTabs.keys());
            this.state.setActiveTab(tabs[tabs.length - 1] || null);
        }

        this.renderTabs();
        this.renderEditor();
    }

    closeAllTabs() {
        for (const [_, tab] of this.state.openTabs) {
            if (tab.autoSaveInterval) {
                clearInterval(tab.autoSaveInterval);
            }
        }
        this.state.openTabs.clear();
        this.state.setActiveTab(null);

        // Reset active tab for all panes
        this.state.panes.forEach(p => {
            p.activeTabId = null;
        });

        this.renderTabs();
        this.renderEditor();
    }

    renderEditor() {
        this.renderPanesStructure();

        for (const pane of this.state.panes) {
            this.renderPaneContent(pane);
        }
    }

    renderPanesStructure() {
        const splitContainer = document.getElementById('split-container');
        if (!splitContainer) return;

        // Check if DOM matches state (optimization)
        const existingPaneIds = Array.from(splitContainer.querySelectorAll('.editor-pane')).map(el => el.id);
        const statePaneIds = this.state.panes.map(p => p.id);
        const structureMatches = existingPaneIds.length === statePaneIds.length &&
            existingPaneIds.every((id, i) => id === statePaneIds[i]);

        if (structureMatches) {
            // Check orientation class
            if (this.state.splitOrientation === 'vertical') {
                if (!splitContainer.classList.contains('split-vertical')) {
                    splitContainer.classList.remove('split-horizontal');
                    splitContainer.classList.add('split-vertical');
                }
            } else {
                if (!splitContainer.classList.contains('split-horizontal')) {
                    splitContainer.classList.remove('split-vertical');
                    splitContainer.classList.add('split-horizontal');
                }
            }
            return;
        }

        // Rebuild structure
        splitContainer.innerHTML = '';
        if (this.state.splitOrientation === 'vertical') {
            splitContainer.classList.add('split-vertical');
            splitContainer.classList.remove('split-horizontal');
        } else {
            splitContainer.classList.add('split-horizontal');
            splitContainer.classList.remove('split-vertical');
        }

        this.state.panes.forEach((pane, index) => {
            const paneEl = document.createElement('div');
            paneEl.className = 'editor-pane';
            paneEl.id = pane.id;
            paneEl.dataset.pane = pane.id;
            paneEl.style.flex = pane.flex.toString();

            paneEl.innerHTML = `
                <div class="pane-header">
                     <div class="editor-toolbar">
                        <button class="btn btn-icon undo-btn" title="Undo (Ctrl+Z)">
                            <span class="icon">↶</span>
                        </button>
                        <button class="btn btn-icon redo-btn" title="Redo (Ctrl+Y)">
                            <span class="icon">↷</span>
                        </button>
                        <span class="toolbar-separator"></span>
                        <button class="btn btn-icon insert-link-btn" title="Insert Link (Ctrl+K)">
                            <span class="icon">🔗</span>
                        </button>
                        <button class="btn btn-icon insert-image-btn" title="Insert Image">
                            <span class="icon">🖼️</span>
                        </button>
                     </div>
                     ${this.state.panes.length > 1 ? `<button class="btn btn-icon close-pane-btn" data-pane="${pane.id}" title="Close Pane">✕</button>` : ''}
                </div>
                <div class="editor-content"></div>
                <div class="editor-mode-selector hidden">
                    <button class="mode-btn" data-mode="raw">Raw</button>
                    <button class="mode-btn" data-mode="side-by-side">Side by Side</button>
                    <button class="mode-btn" data-mode="formatted">Formatted</button>
                    <button class="mode-btn" data-mode="rendered">Rendered</button>
                </div>
            `;

            // Attach toolbar events
            paneEl.querySelector('.undo-btn')?.addEventListener('click', (e) => {
                e.stopPropagation();
                if (pane.activeTabId) {
                    this.state.setActiveTab(pane.activeTabId);
                    this.state.activePaneId = pane.id;
                    this.renderTabs();
                    this.undo();
                }
            });

            paneEl.querySelector('.redo-btn')?.addEventListener('click', (e) => {
                e.stopPropagation();
                if (pane.activeTabId) {
                    this.state.setActiveTab(pane.activeTabId);
                    this.state.activePaneId = pane.id;
                    this.renderTabs();
                    this.redo();
                }
            });

            paneEl.querySelector('.insert-link-btn')?.addEventListener('click', (e) => {
                e.stopPropagation();
                if (pane.activeTabId) {
                    this.state.setActiveTab(pane.activeTabId);
                    this.state.activePaneId = pane.id;
                    this.renderTabs();
                    this.showInsertLinkModal();
                }
            });

            paneEl.querySelector('.insert-image-btn')?.addEventListener('click', (e) => {
                e.stopPropagation();
                if (pane.activeTabId) {
                    this.state.setActiveTab(pane.activeTabId);
                    this.state.activePaneId = pane.id;
                    this.renderTabs();
                    this.showInsertImageModal();
                }
            });

            // Mode selector logic to be handled in renderPaneContent or event listeners
            const modeBtns = paneEl.querySelectorAll('.mode-btn');
            modeBtns.forEach(btn => {
                btn.addEventListener('click', (e) => {
                    e.stopPropagation();
                    const mode = (e.target as HTMLElement).getAttribute('data-mode') as any;
                    if (mode) {
                        this.state.editorMode = mode;
                        this.renderEditor();
                        this.savePreferences();
                    }
                });
            });

            // Activate pane on click
            paneEl.addEventListener('click', () => {
                if (this.state.activePaneId !== pane.id) {
                    this.state.activePaneId = pane.id;
                    this.updatePaneActiveState();
                }
            });

            // Close pane
            const closeBtn = paneEl.querySelector('.close-pane-btn');
            closeBtn?.addEventListener('click', (e) => {
                e.stopPropagation();
                this.closePane(pane.id);
            });

            splitContainer.appendChild(paneEl);

            // Add resizer
            if (index < this.state.panes.length - 1) {
                const resizer = document.createElement('div');
                resizer.className = 'pane-resizer';
                if (this.state.splitOrientation === 'horizontal') resizer.classList.add('horizontal');

                // Add resizing logic listeners here or in setupSplitPane
                this.setupResizer(resizer, pane, this.state.panes[index + 1]);

                splitContainer.appendChild(resizer);
            }
        });

        this.updatePaneActiveState();
    }

    renderPaneContent(pane: any) {
        const container = document.getElementById(pane.id);
        if (!container) return;

        const content = container.querySelector('.editor-content') as HTMLElement;
        const modeSelector = container.querySelector('.editor-mode-selector');

        if (!this.state.activeTabId && (!pane.activeTabId)) { // Global active or pane active check?
            // If this pane has no active tab
            content.innerHTML = `
                <div class="empty-state">
                    <h2>No file open</h2>
                    <p>Select a file from the sidebar</p>
                </div>
            `;
            modeSelector?.classList.add('hidden');
            return;
        }

        const tabId = pane.activeTabId;
        if (!tabId) {
            content.innerHTML = `
                <div class="empty-state">
                    <h2>No file open</h2>
                    <p>Select a file</p>
                </div>
            `;
            modeSelector?.classList.add('hidden');
            return;
        }

        const tab = this.state.getTab(tabId);
        if (!tab) return; // Should not happen

        // Ensure auto-save is running (logic previously in renderEditor)
        if (!tab.autoSaveInterval) {
            tab.autoSaveInterval = window.setInterval(() => {
                if (tab.isDirty) {
                    this.saveFile(tab.id);
                }
            }, 5000);
        }

        // Show/Hide mode selector based on file type
        if (modeSelector) {
            if (tab.fileType === 'markdown') {
                modeSelector.classList.remove('hidden');
                modeSelector.querySelectorAll('.mode-btn').forEach(btn => {
                    if (btn.getAttribute('data-mode') === this.state.editorMode) {
                        btn.classList.add('active');
                    } else {
                        btn.classList.remove('active');
                    }
                });
            } else {
                modeSelector.classList.add('hidden');
            }
        }

        // Handle different file types
        if (tab.fileType === 'image') {
            this.renderImageViewer(content, tab);
            return;
        } else if (tab.fileType === 'pdf') {
            this.renderPdfViewer(content, tab);
            return;
        } else if (tab.fileType === 'audio') {
            this.renderAudioViewer(content, tab);
            return;
        } else if (tab.fileType === 'video') {
            this.renderVideoViewer(content, tab);
            return;
        } else if (tab.fileType === 'other') {
            this.renderUnsupportedFile(content, tab);
            return;
        } else if (tab.fileType === 'text') {
            this.renderRawEditor(content, tab, pane.id); // Add paneId arg
            return;
        }

        // For markdown
        switch (this.state.editorMode) {
            case 'raw':
                this.renderRawEditor(content, tab, pane.id);
                break;
            case 'side-by-side':
                this.renderSideBySideEditor(content, tab, pane.id);
                break;
            case 'formatted':
                this.renderFormattedEditor(content, tab, pane.id);
                break;
            case 'rendered':
                this.renderRenderedEditor(content, tab, pane.id);
                break;
        }
    }

    renderRawEditor(container: HTMLElement, tab: Tab, paneId: string) {
        // If it's a code file (non-markdown text), show with syntax highlighting
        if (tab.fileType === 'text') {
            const ext = tab.fileName.split('.').pop() || 'txt';
            const language = ext === 'rs' ? 'rust' : (ext === 'ts' ? 'typescript' : (ext === 'js' ? 'javascript' : ext));

            container.innerHTML = `
                <div class="code-viewer">
                    <pre><code class="language-${language}">${this.escapeHtml(tab.content)}</code></pre>
                </div>
            `;

            container.querySelectorAll('pre code').forEach((block) => {
                hljs.highlightElement(block as HTMLElement);
            });
            return;
        }

        container.innerHTML = `<textarea class="editor-raw" id="editor-textarea-${paneId}">${tab.content}</textarea>`;

        const textarea = container.querySelector("textarea") as HTMLTextAreaElement;
        if (textarea) {
            textarea.addEventListener('input', () => {
                const newContent = textarea.value;
                tab.undoManager?.recordChange(newContent);
                tab.content = newContent;
                tab.isDirty = true;
                this.renderTabs();
                this.updateUndoRedoButtons();
            });

            // Setup drag-and-drop for images/files
            this.setupEditorDropZone(container, textarea);
        }
    }



    renderSideBySideEditor(container: HTMLElement, tab: Tab, paneId: string) {
        // TODO: Make layout configurable:
        // - Orientation: vertical (left/right) vs horizontal (top/bottom) - use splitOrientation setting
        // - Order: rendered-first vs raw-first - add user preference
        // - Currently: rendered (left) | raw (right)
        container.innerHTML = `
            <div class="editor-side-by-side">
                <div class="editor-preview markdown-content" id="preview-pane-${paneId}"></div>
                <div>
                    <textarea class="editor-raw" id="editor-textarea-${paneId}">${tab.content}</textarea>
                </div>
            </div>
        `;

        const textarea = container.querySelector('textarea') as HTMLTextAreaElement;
        const preview = container.querySelector(`#preview-pane-${paneId}`) as HTMLElement;

        if (textarea && preview) {
            const updatePreview = debounce(async () => {
                preview.innerHTML = await this.renderMarkdown(textarea.value);
                this.setupWikiLinkHandlers(preview, tab.filePath);
            }, 300);

            textarea.addEventListener('input', () => {
                const newContent = textarea.value;
                tab.undoManager?.recordChange(newContent);
                tab.content = newContent;
                tab.isDirty = true;
                updatePreview();
                this.renderTabs();
                this.updateUndoRedoButtons();
            });

            updatePreview();

            // Setup drag-and-drop for images/files
            this.setupEditorDropZone(container, textarea);
        }
    }

    /**
     * Setup click handlers for wiki links in a container element
     */
    setupWikiLinkHandlers(container: HTMLElement, currentFilePath: string) {
        // Handle clicks on wiki links
        container.querySelectorAll('a.wiki-link').forEach((link) => {
            const anchor = link as HTMLAnchorElement;

            // Prevent default link behavior and handle navigation
            anchor.addEventListener('click', async (e) => {
                e.preventDefault();
                e.stopPropagation();

                const originalLink = anchor.getAttribute('data-original-link');
                if (!originalLink || !this.state.currentVaultId) return;

                // Extract the link target (without fragment)
                const [linkTarget, fragment] = originalLink.split('#');

                try {
                    // Resolve the wiki link to get the actual file path
                    const resolved = await this.api.resolveWikiLink(
                        this.state.currentVaultId,
                        linkTarget,
                        currentFilePath
                    );

                    if (resolved.exists) {
                        // Open the file
                        await this.openFile(resolved.path);

                        // If there's a fragment (header link), scroll to it after a brief delay
                        if (fragment) {
                            setTimeout(() => {
                                this.scrollToFragment(fragment);
                            }, 100);
                        }
                    } else {
                        // File doesn't exist - offer to create it
                        if (confirm(`"${linkTarget}" doesn't exist. Would you like to create it?`)) {
                            await this.createAndOpenFile(resolved.path);
                        }
                    }
                } catch (error) {
                    console.error('Failed to navigate to wiki link:', error);
                    alert('Failed to navigate to link: ' + error);
                }
            });

            // Add visual feedback for hover
            anchor.style.cursor = 'pointer';
        });

        // Also handle wiki embeds (images) that might need special handling
        container.querySelectorAll('img.wiki-embed').forEach((img) => {
            const imgEl = img as HTMLImageElement;
            const originalLink = imgEl.getAttribute('data-original-link');

            if (originalLink && this.state.currentVaultId) {
                // Update the src to use the raw file endpoint with resolved path
                this.api.resolveWikiLink(this.state.currentVaultId, originalLink, currentFilePath)
                    .then(resolved => {
                        if (resolved.exists) {
                            imgEl.src = `/api/vaults/${this.state.currentVaultId}/raw/${resolved.path}`;
                        }
                    })
                    .catch(err => console.error('Failed to resolve image path:', err));
            }
        });
    }

    /**
     * Scroll to a heading or block reference in the current editor/preview
     */
    scrollToFragment(fragment: string) {
        // Look for header with matching ID or text
        const container = document.getElementById('pane-1');
        if (!container) return;

        // Try to find element by ID first (for explicit IDs)
        let target = container.querySelector(`#${CSS.escape(fragment)}`);

        // If not found, look for headings that match the fragment text
        if (!target) {
            const headings = container.querySelectorAll('h1, h2, h3, h4, h5, h6');
            headings.forEach((heading) => {
                if (target) return; // Already found
                const headingText = heading.textContent?.toLowerCase().replace(/\s+/g, '-') || '';
                if (headingText === fragment.toLowerCase() || heading.id === fragment) {
                    target = heading;
                }
            });
        }

        // If still not found, look for block references
        if (!target && fragment.startsWith('^')) {
            target = container.querySelector(`[data-block-id="${fragment.substring(1)}"]`);
        }

        if (target) {
            target.scrollIntoView({ behavior: 'smooth', block: 'start' });
            // Add a brief highlight effect
            target.classList.add('highlight-target');
            setTimeout(() => target?.classList.remove('highlight-target'), 2000);
        }
    }

    /**
     * Create a new file and open it
     */
    async createAndOpenFile(filePath: string) {
        if (!this.state.currentVaultId) return;

        try {
            // Ensure the file has .md extension
            const path = filePath.endsWith('.md') ? filePath : `${filePath}.md`;

            // Create the file with a default header
            const fileName = path.split('/').pop()?.replace('.md', '') || 'Untitled';
            await this.api.createFile(this.state.currentVaultId, path, `# ${fileName}\n\n`);

            // Refresh file tree
            await this.loadFileTree();

            // Open the new file
            await this.openFile(path);
        } catch (error) {
            console.error('Failed to create file:', error);
            alert('Failed to create file: ' + error);
        }
    }

    renderFormattedEditor(container: HTMLElement, tab: Tab, paneId: string) {
        container.innerHTML = `<div class="editor-formatted language-markdown" id="editor-formatted-${paneId}"></div>`;

        const editor = container.querySelector(`#editor-formatted-${paneId}`) as HTMLElement;
        if (editor) {
            // Destroy existing jar for this pane if needed (CodeJar doesn't have destroy?)
            // We just override the reference

            const jar = CodeJar(editor, (editor: HTMLElement) => {
                hljs.highlightElement(editor);
            });

            jar.updateCode(tab.content, false);
            jar.onUpdate((code: string) => {
                tab.undoManager?.recordChange(code);
                tab.content = code;
                tab.isDirty = true;
                this.renderTabs();
                this.updateUndoRedoButtons();
            });

            this.paneJars.set(paneId, jar);

            // Setup drag-and-drop for images/files
            this.setupEditorDropZone(container);
        }
    }

    async renderRenderedEditor(container: HTMLElement, tab: Tab, paneId: string) {
        container.innerHTML = '<div class="loading">Loading WYSIWYG Editor...</div>';

        // Render markdown to HTML via backend
        const html = await this.renderMarkdown(tab.content);
        if (paneId !== this.state.activePaneId && this.state.getTab(this.state.panes.find(p => p.id === paneId)?.activeTabId || '')?.id !== tab.id) {
            // Basic check to see if we still care?
        }

        // Setup container
        container.innerHTML = `<div id="editor-wysiwyg-${paneId}" class="editor-wysiwyg"></div>`;
        const editorEl = container.querySelector(`#editor-wysiwyg-${paneId}`);

        if (editorEl) {
            // @ts-ignore
            const quill = new Quill(editorEl, {
                theme: 'snow',
                modules: {
                    toolbar: [
                        [{ 'header': [1, 2, 3, false] }],
                        ['bold', 'italic', 'underline', 'strike', 'blockquote', 'code-block'],
                        [{ 'list': 'ordered' }, { 'list': 'bullet' }],
                        ['link', 'image'],
                        ['clean']
                    ]
                }
            });

            // Set content
            // @ts-ignore
            quill.clipboard.dangerouslyPasteHTML(html);

            // Track changes
            // @ts-ignore
            quill.on('text-change', debounce((_delta: any, _oldDelta: any, source: string) => {
                if (source !== 'user') return;
                // @ts-ignore
                const newHtml = quill.root.innerHTML;
                const markdown = this.htmlToMarkdown(newHtml);
                tab.undoManager?.recordChange(markdown);
                tab.content = markdown;
                tab.isDirty = true;
                this.renderTabs();
                this.updateUndoRedoButtons();
            }, 500));

            this.paneQuills.set(paneId, quill);

            // Setup drag-and-drop for images/files
            this.setupEditorDropZone(container);
        }
    }

    htmlToMarkdown(html: string): string {
        // @ts-ignore
        const turndownService = new TurndownService({
            headingStyle: 'atx',
            codeBlockStyle: 'fenced'
        });

        // Add rule for wiki links (heuristic: internal links don't start with http)
        turndownService.addRule('wikiLink', {
            filter: function (node: any) {
                return node.nodeName === 'A' && node.getAttribute('href') && !node.getAttribute('href').startsWith('http');
            },
            replacement: function (content: any, node: any) {
                const href = node.getAttribute('href');
                if (href === content) return `[[${href}]]`;
                return `[[${href}|${content}]]`;
            }
        });

        return turndownService.turndown(html);
    }

    // Undo/Redo functionality
    undo(): void {
        if (!this.state.activeTabId) return;
        const tab = this.state.getTab(this.state.activeTabId);
        if (!tab || !tab.undoManager) return;

        const content = tab.undoManager.undo();
        if (content !== null) {
            tab.content = content;
            tab.isDirty = true;
            this.renderTabs();
            this.updateEditorContent(content);
            this.updateUndoRedoButtons();
        }
    }

    redo(): void {
        if (!this.state.activeTabId) return;
        const tab = this.state.getTab(this.state.activeTabId);
        if (!tab || !tab.undoManager) return;

        const content = tab.undoManager.redo();
        if (content !== null) {
            tab.content = content;
            tab.isDirty = true;
            this.renderTabs();
            this.updateEditorContent(content);
            this.updateUndoRedoButtons();
        }
    }

    private updateEditorContent(content: string): void {
        const tab = this.state.activeTabId ? this.state.getTab(this.state.activeTabId) : null;
        if (!tab) return;

        const paneId = tab.pane;
        const paneContainer = document.getElementById(paneId);
        if (!paneContainer) return;

        const currentFilePath = tab.filePath || '';

        // Update textarea if present (raw or side-by-side mode)
        const textarea = paneContainer.querySelector('textarea.editor-raw') as HTMLTextAreaElement;
        if (textarea) {
            textarea.value = content;
            // Also update preview in side-by-side mode
            const preview = paneContainer.querySelector('.editor-preview') as HTMLElement;
            if (preview) {
                this.renderMarkdown(content).then(html => {
                    preview.innerHTML = html;
                    this.setupWikiLinkHandlers(preview, currentFilePath);
                });
            }
            return;
        }

        // Update CodeJar if present (formatted mode)
        const jar = this.paneJars.get(paneId);
        if (jar) {
            jar.updateCode(content, false);
            return;
        }

        // Update Quill if present (rendered mode)
        const quill = this.paneQuills.get(paneId);
        if (quill) {
            this.renderMarkdown(content).then(html => {
                quill.clipboard.dangerouslyPasteHTML(html);
            });
            return;
        }
    }



    updateUndoRedoButtons(): void {
        const tab = this.state.activeTabId ? this.state.getTab(this.state.activeTabId) : null;
        const undoBtn = document.getElementById('undo-btn');
        const redoBtn = document.getElementById('redo-btn');

        if (undoBtn) {
            undoBtn.classList.toggle('disabled', !tab?.undoManager?.canUndo());
        }
        if (redoBtn) {
            redoBtn.classList.toggle('disabled', !tab?.undoManager?.canRedo());
        }
    }

    renderImageViewer(container: HTMLElement, tab: Tab) {
        container.innerHTML = `
            <div class="image-viewer">
                <div class="image-controls">
                    <button class="btn btn-icon" id="zoom-in" title="Zoom In">+</button>
                    <button class="btn btn-icon" id="zoom-out" title="Zoom Out">-</button>
                    <button class="btn btn-icon" id="zoom-reset" title="Reset">⟲</button>
                    <span class="zoom-level" id="zoom-level">100%</span>
                </div>
                <div class="image-container" id="image-container">
                    <img src="${tab.content}" alt="${tab.fileName}" id="image-display">
                </div>
            </div>
        `;

        let zoom = 1.0;
        const img = container.querySelector('#image-display') as HTMLImageElement;
        const zoomLevel = container.querySelector('#zoom-level') as HTMLSpanElement;
        const imageContainer = container.querySelector('#image-container') as HTMLDivElement;

        const updateZoom = () => {
            if (img) {
                img.style.transform = `scale(${zoom})`;
                zoomLevel.textContent = `${Math.round(zoom * 100)}%`;
            }
        };

        container.querySelector('#zoom-in')?.addEventListener('click', () => {
            zoom = Math.min(zoom + 0.25, 5);
            updateZoom();
        });

        container.querySelector('#zoom-out')?.addEventListener('click', () => {
            zoom = Math.max(zoom - 0.25, 0.25);
            updateZoom();
        });

        container.querySelector('#zoom-reset')?.addEventListener('click', () => {
            zoom = 1.0;
            updateZoom();
        });

        // Pan functionality
        let isPanning = false;
        let startX = 0;
        let startY = 0;
        let scrollLeft = 0;
        let scrollTop = 0;

        imageContainer.addEventListener('mousedown', (e) => {
            isPanning = true;
            startX = e.pageX - imageContainer.offsetLeft;
            startY = e.pageY - imageContainer.offsetTop;
            scrollLeft = imageContainer.scrollLeft;
            scrollTop = imageContainer.scrollTop;
            imageContainer.style.cursor = 'grabbing';
        });

        imageContainer.addEventListener('mouseleave', () => {
            isPanning = false;
            imageContainer.style.cursor = 'grab';
        });

        imageContainer.addEventListener('mouseup', () => {
            isPanning = false;
            imageContainer.style.cursor = 'grab';
        });

        imageContainer.addEventListener('mousemove', (e) => {
            if (!isPanning) return;
            e.preventDefault();
            const x = e.pageX - imageContainer.offsetLeft;
            const y = e.pageY - imageContainer.offsetTop;
            const walkX = (x - startX) * 2;
            const walkY = (y - startY) * 2;
            imageContainer.scrollLeft = scrollLeft - walkX;
            imageContainer.scrollTop = scrollTop - walkY;
        });
    }

    renderPdfViewer(container: HTMLElement, tab: Tab) {
        container.innerHTML = `
            <div class="pdf-viewer" style="height: 100%; display: flex; flex-direction: column;">
                <div class="pdf-controls" style="padding: 5px; background: var(--bg-secondary); border-bottom: 1px solid var(--border-color); display: flex; gap: 10px; align-items: center; flex-wrap: wrap;">
                    <button class="btn btn-icon pdf-prev-btn" title="Previous Page">◀</button>
                    <span>
                        <span class="pdf-page-num">1</span> / <span class="pdf-page-count">--</span>
                    </span>
                    <button class="btn btn-icon pdf-next-btn" title="Next Page">▶</button>
                    <span class="toolbar-separator"></span>
                    <div style="display: flex; gap: 5px;">
                        <input type="text" class="pdf-search-input" placeholder="Search..." style="width: 100px; padding: 2px;">
                        <button class="btn btn-icon pdf-search-btn" title="Search">🔍</button>
                    </div>
                    <span class="toolbar-separator"></span>
                    <button class="btn btn-icon pdf-zoom-out-btn" title="Zoom Out">-</button>
                    <span class="pdf-zoom-level">100%</span>
                    <button class="btn btn-icon pdf-zoom-in-btn" title="Zoom In">+</button>
                    <span class="toolbar-separator"></span>
                    <button class="btn btn-icon pdf-meta-btn" title="Metadata">ℹ</button>
                </div>
                <div class="pdf-canvas-container" style="flex: 1; overflow: auto; display: flex; justify-content: center; padding: 20px; background: var(--bg-primary);">
                    <canvas class="pdf-canvas"></canvas>
                </div>
            </div>
        `;

        const canvas = container.querySelector('.pdf-canvas') as HTMLCanvasElement;
        const pageNumSpan = container.querySelector('.pdf-page-num') as HTMLElement;
        const pageCountSpan = container.querySelector('.pdf-page-count') as HTMLElement;
        const zoomLevelSpan = container.querySelector('.pdf-zoom-level') as HTMLElement;

        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        // Ensure state exists
        let state = this.pdfStates.get(tab.id);

        // Reset state if URL changed
        if (state && state.url !== tab.content) {
            state = undefined;
        }

        if (!state) {
            state = {
                pdfDoc: null,
                pageNum: 1,
                pageRendering: false,
                pageNumPending: null,
                scale: 1.0,
                canvas: canvas,
                ctx: ctx,
                url: tab.content
            };
            this.pdfStates.set(tab.id, state);
        } else {
            // Update references
            state.canvas = canvas;
            state.ctx = ctx;
        }

        const renderPage = (num: number) => {
            state.pageRendering = true;

            state.pdfDoc.getPage(num).then((page: any) => {
                const viewport = page.getViewport({ scale: state.scale });
                state.canvas.height = viewport.height;
                state.canvas.width = viewport.width;

                const renderContext = {
                    canvasContext: state.ctx,
                    viewport: viewport
                };

                const renderTask = page.render(renderContext);

                renderTask.promise.then(() => {
                    state.pageRendering = false;
                    if (state.pageNumPending !== null) {
                        renderPage(state.pageNumPending);
                        state.pageNumPending = null;
                    }
                });
            });

            if (pageNumSpan) pageNumSpan.textContent = num.toString();
            if (zoomLevelSpan) zoomLevelSpan.textContent = `${Math.round(state.scale * 100)}%`;
        };

        const queueRenderPage = (num: number) => {
            if (state.pageRendering) {
                state.pageNumPending = num;
            } else {
                renderPage(num);
            }
        };

        // Event Listeners
        container.querySelector('.pdf-prev-btn')?.addEventListener('click', () => {
            if (state && state.pageNum > 1) {
                state.pageNum--;
                queueRenderPage(state.pageNum);
            }
        });

        container.querySelector('.pdf-next-btn')?.addEventListener('click', () => {
            if (state && state.pageNum < state.pdfDoc.numPages) {
                state.pageNum++;
                queueRenderPage(state.pageNum);
            }
        });

        container.querySelector('.pdf-zoom-in-btn')?.addEventListener('click', () => {
            if (state) {
                state.scale += 0.25;
                queueRenderPage(state.pageNum);
            }
        });

        container.querySelector('.pdf-zoom-out-btn')?.addEventListener('click', () => {
            if (state && state.scale > 0.25) {
                state.scale -= 0.25;
                queueRenderPage(state.pageNum);
            }
        });


        container.querySelector('.pdf-meta-btn')?.addEventListener('click', () => {
            if (state && state.pdfDoc) {
                state.pdfDoc.getMetadata().then((data: any) => {
                    const info = data.info || {};
                    let text = "PDF Metadata:\n";
                    text += `Title: ${info.Title || '-'}\n`;
                    text += `Author: ${info.Author || '-'}\n`;
                    text += `Subject: ${info.Subject || '-'}\n`;
                    text += `Creator: ${info.Creator || '-'}\n`;
                    text += `Producer: ${info.Producer || '-'}\n`;
                    text += `CreationDate: ${info.CreationDate || '-'}`;
                    alert(text);
                });
            }
        });

        const performSearch = async () => {
            const input = container.querySelector('.pdf-search-input') as HTMLInputElement;
            const query = input.value.trim().toLowerCase();
            if (!query || !state || !state.pdfDoc) return;

            const btn = container.querySelector('.pdf-search-btn') as HTMLElement;
            const originalText = btn.textContent;
            btn.textContent = '...';

            let found = false;
            // Start from current page + 1
            for (let i = state.pageNum + 1; i <= state.pdfDoc.numPages; i++) {
                const page = await state.pdfDoc.getPage(i);
                const textContent = await page.getTextContent();
                const text = textContent.items.map((item: any) => item.str).join(' ').toLowerCase();

                if (text.includes(query)) {
                    state.pageNum = i;
                    queueRenderPage(i);
                    found = true;
                    break;
                }
            }

            // If not found, wrap around
            if (!found) {
                for (let i = 1; i <= state.pageNum; i++) {
                    const page = await state.pdfDoc.getPage(i);
                    const textContent = await page.getTextContent();
                    const text = textContent.items.map((item: any) => item.str).join(' ').toLowerCase();

                    if (text.includes(query)) {
                        state.pageNum = i;
                        queueRenderPage(i);
                        found = true;
                        break;
                    }
                }
            }

            btn.textContent = originalText;

            if (!found) {
                alert('Text not found');
            }
        };

        container.querySelector('.pdf-search-btn')?.addEventListener('click', performSearch);
        container.querySelector('.pdf-search-input')?.addEventListener('keypress', (e) => {
            if ((e as KeyboardEvent).key === 'Enter') performSearch();
        });

        // Load logic
        if (!state.pdfDoc) {
            const loadingTask = pdfjsLib.getDocument(state.url);
            loadingTask.promise.then((pdfDoc_: any) => {
                state.pdfDoc = pdfDoc_;
                if (pageCountSpan) pageCountSpan.textContent = state.pdfDoc.numPages;
                renderPage(state.pageNum);
            }, (reason: any) => {
                console.error('Error loading PDF:', reason);
                container.innerHTML = `<div class="error" style="padding: 20px;">Error loading PDF: ${reason}</div>`;
            });
        } else {
            if (pageCountSpan) pageCountSpan.textContent = state.pdfDoc.numPages;
            renderPage(state.pageNum);
        }
    }

    renderAudioViewer(container: HTMLElement, tab: Tab) {
        container.innerHTML = `
            <div class="media-viewer">
                <div class="media-container">
                    <div class="media-icon">🎵</div>
                    <h3>${tab.fileName}</h3>
                    <audio controls src="${tab.content}" style="width: 100%; max-width: 500px; margin-top: 20px;">
                        Your browser does not support the audio element.
                    </audio>
                </div>
            </div>
        `;
    }

    renderVideoViewer(container: HTMLElement, tab: Tab) {
        container.innerHTML = `
            <div class="media-viewer">
                <div class="media-container">
                    <h3>${tab.fileName}</h3>
                    <video controls src="${tab.content}" style="max-width: 100%; max-height: 80vh;">
                        Your browser does not support the video element.
                    </video>
                </div>
            </div>
        `;
    }

    renderUnsupportedFile(container: HTMLElement, tab: Tab) {
        container.innerHTML = `
            <div class="unsupported-file">
                <h2>Unsupported File Type</h2>
                <p>This file type cannot be previewed in the browser.</p>
                <p><strong>File:</strong> ${tab.fileName}</p>
                <a href="${tab.content}" download="${tab.fileName}" class="btn btn-primary">Download File</a>
            </div>
        `;
    }

    async renderMarkdown(content: string): Promise<string> {
        try {
            return await this.api.renderMarkdown(content);
        } catch (e) {
            console.error('Markdown render error:', e);
            return `<p class="error">Failed to render markdown: ${e}</p>`;
        }
    }

    // Bulk Operations
    updateBulkActionsToolbar() {
        const toolbar = document.getElementById('bulk-actions-toolbar');
        const selectionCount = document.getElementById('selection-count');

        if (!toolbar || !selectionCount) return;

        const count = this.state.selectedFiles.size;
        selectionCount.textContent = `${count} selected`;

        if (count > 0) {
            toolbar.classList.add('active');
        } else {
            toolbar.classList.remove('active');
        }
    }

    selectAllFiles() {
        const checkboxes = document.querySelectorAll('.file-checkbox') as NodeListOf<HTMLInputElement>;
        checkboxes.forEach(checkbox => {
            const path = checkbox.getAttribute('data-path');
            if (path) {
                this.state.selectedFiles.add(path);
                checkbox.checked = true;
                checkbox.closest('.file-tree-item')?.classList.add('selected');
            }
        });
        this.updateBulkActionsToolbar();
    }

    deselectAllFiles() {
        this.state.selectedFiles.clear();
        const checkboxes = document.querySelectorAll('.file-checkbox') as NodeListOf<HTMLInputElement>;
        checkboxes.forEach(checkbox => {
            checkbox.checked = false;
            checkbox.closest('.file-tree-item')?.classList.remove('selected');
        });
        this.updateBulkActionsToolbar();
    }

    async bulkDownload() {
        if (this.state.selectedFiles.size === 0 || !this.state.currentVaultId) return;

        const paths = Array.from(this.state.selectedFiles);

        try {
            await this.api.downloadZip(this.state.currentVaultId, paths);
        } catch (error) {
            console.error('Bulk download failed:', error);
            alert('Failed to download files: ' + error);
        }
    }

    async bulkDelete() {
        if (this.state.selectedFiles.size === 0 || !this.state.currentVaultId) return;

        const count = this.state.selectedFiles.size;
        const confirmed = confirm(`Are you sure you want to delete ${count} file(s)? This action cannot be undone.`);

        if (!confirmed) return;

        const paths = Array.from(this.state.selectedFiles);
        let successCount = 0;
        let failCount = 0;

        for (const path of paths) {
            try {
                await this.api.deleteFile(this.state.currentVaultId, path);
                successCount++;
            } catch (error) {
                console.error(`Failed to delete ${path}:`, error);
                failCount++;
            }
        }

        // Clear selection and refresh tree
        this.deselectAllFiles();
        await this.loadFileTree();

        if (failCount > 0) {
            alert(`Deleted ${successCount} file(s). Failed to delete ${failCount} file(s).`);
        } else {
            alert(`Successfully deleted ${successCount} file(s).`);
        }
    }

    setupEventListeners() {
        // Vault selector
        const vaultSelect = document.getElementById('vault-select') as HTMLSelectElement;
        vaultSelect?.addEventListener('change', (e) => {
            const target = e.target as HTMLSelectElement;
            this.switchVault(target.value);
        });

        // Add vault button
        const addVaultBtn = document.getElementById('add-vault-btn');
        addVaultBtn?.addEventListener('click', () => {
            this.showModal('add-vault-modal');
        });

        // Add vault form
        const addVaultForm = document.getElementById('add-vault-form') as HTMLFormElement;
        const vaultPathInput = document.getElementById('vault-path') as HTMLInputElement;
        const vaultPathPicker = document.getElementById('vault-path-picker') as HTMLInputElement | null;
        const vaultPathBrowse = document.getElementById('vault-path-browse');

        if (vaultPathBrowse && vaultPathPicker && vaultPathInput) {
            vaultPathBrowse.addEventListener('click', () => {
                vaultPathPicker.value = '';
                vaultPathPicker.click();
            });

            vaultPathPicker.addEventListener('change', (e) => {
                const input = e.target as HTMLInputElement;
                const files = input.files;
                if (!files || files.length === 0) return;

                const first: any = files[0];
                const explicitPath = first.path as string | undefined;
                if (explicitPath) {
                    vaultPathInput.value = explicitPath;
                    return;
                }

                const relative = first.webkitRelativePath as string | undefined;
                if (relative) {
                    const parts = relative.split(/[\\/]/);
                    if (parts.length > 1) {
                        vaultPathInput.value = parts[0];
                    }
                }
            });
        }

        const vaultList = document.getElementById('vault-list');
        vaultList?.addEventListener('click', async (e) => {
            const target = e.target as HTMLElement;
            const btn = target.closest('[data-action="delete-vault"]') as HTMLElement | null;
            if (!btn) return;
            const vaultId = btn.getAttribute('data-vault-id');
            if (!vaultId) return;

            const proceed = confirm('Remove this vault? This will detach it from Obsidian Host but will not delete files on disk.');
            if (!proceed) return;

            try {
                await this.api.deleteVault(vaultId);
                if (this.state.currentVaultId === vaultId) {
                    this.state.setVault(null);
                    this.closeAllTabs();
                }
                await this.loadVaults();
            } catch (error) {
                alert('Failed to delete vault: ' + error);
            }
        });

        addVaultForm?.addEventListener('submit', async (e) => {
            e.preventDefault();
            const formData = new FormData(addVaultForm);
            const name = formData.get('name') as string;
            const path = formData.get('path') as string;

            try {
                const vault = await this.api.createVault(name, path);
                await this.loadVaults();
                this.switchVault(vault.id);
                this.hideModal('add-vault-modal');
                addVaultForm.reset();
            } catch (error) {
                alert('Failed to create vault: ' + error);
            }
        });

        // Modal close buttons
        document.querySelectorAll('[data-close-modal]').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const target = e.target as HTMLElement;
                const modalId = target.getAttribute('data-close-modal');
                if (modalId) this.hideModal(modalId);
            });
        });

        // Search
        const searchInput = document.getElementById('search-input') as HTMLInputElement;
        searchInput?.addEventListener('input', (e) => {
            if (this.state.searchDebounce) {
                clearTimeout(this.state.searchDebounce);
            }

            const target = e.target as HTMLInputElement;
            const query = target.value.trim();

            if (query.length < 2) return;

            this.state.searchDebounce = window.setTimeout(() => {
                this.performSearch(query);
            }, 300);
        });

        // Theme toggle
        const themeToggleBtn = document.getElementById('theme-toggle-btn');
        themeToggleBtn?.addEventListener('click', () => {
            document.body.classList.toggle('theme-dark');
        });

        // Plugin Manager button
        const pluginManagerBtn = document.getElementById('plugin-manager-btn');
        pluginManagerBtn?.addEventListener('click', async () => {
            this.showModal('plugin-manager-modal');
            await this.loadPlugins();
        });

        // Editor mode buttons
        document.querySelectorAll('.mode-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const target = e.target as HTMLElement;
                const mode = target.getAttribute('data-mode') as any;
                if (mode) {
                    this.state.editorMode = mode;

                    // Update active state
                    target.parentElement?.querySelectorAll('.mode-btn').forEach(b => b.classList.remove('active'));
                    target.classList.add('active');

                    this.renderEditor();
                }
            });
        });

        // Download button
        const downloadBtn = document.getElementById('download-btn');
        downloadBtn?.addEventListener('click', async () => {
            if (!this.state.activeTabId || !this.state.currentVaultId) {
                alert('No file is currently open');
                return;
            }

            const tab = this.state.getTab(this.state.activeTabId);
            if (!tab) return;

            try {
                await this.api.downloadFile(this.state.currentVaultId, tab.filePath);
            } catch (error) {
                console.error('Download failed:', error);
                alert('Failed to download file: ' + error);
            }
        });

        // Properties panel toggle
        const propertiesToggleBtn = document.getElementById('properties-toggle-btn');
        propertiesToggleBtn?.addEventListener('click', () => {
            this.togglePropertiesPanel();
        });

        const closePropertiesBtn = document.getElementById('close-properties');
        closePropertiesBtn?.addEventListener('click', () => {
            this.hidePropertiesPanel();
        });

        // Properties panel actions
        const addPropertyBtn = document.getElementById('add-property-btn');
        addPropertyBtn?.addEventListener('click', () => {
            this.addProperty();
        });

        const savePropertiesBtn = document.getElementById('save-properties-btn');
        savePropertiesBtn?.addEventListener('click', async () => {
            await this.saveProperties();
        });

        // New file/folder (toolbar)
        const newFileBtn = document.getElementById('new-file-btn');
        newFileBtn?.addEventListener('click', async () => {
            if (!this.state.currentVaultId) {
                alert('Please select a vault first');
                return;
            }
            const fileName = prompt('Enter file name:');
            if (!fileName) return;
            const basePath = this.getCreationBasePath();
            const filePath = basePath ? `${basePath}/${fileName}` : fileName;
            try {
                await this.api.createFile(this.state.currentVaultId, filePath, '');
                await this.loadFileTree();
                await this.openFile(filePath);
            } catch (error) {
                console.error('Failed to create file:', error);
                alert('Failed to create file: ' + error);
            }
        });

        const newFolderBtn = document.getElementById('new-folder-btn');
        newFolderBtn?.addEventListener('click', async () => {
            if (!this.state.currentVaultId) {
                alert('Please select a vault first');
                return;
            }
            const folderName = prompt('Enter folder name:');
            if (!folderName) return;
            const basePath = this.getCreationBasePath();
            const folderPath = basePath ? `${basePath}/${folderName}` : folderName;
            try {
                await this.api.createDirectory(this.state.currentVaultId, folderPath);
                await this.loadFileTree();
            } catch (error) {
                console.error('Failed to create folder:', error);
                alert('Failed to create folder: ' + error);
            }
        });

        const uniqueNoteBtn = document.getElementById('unique-note-btn');
        uniqueNoteBtn?.addEventListener('click', async () => {
            if (!this.state.currentVaultId) {
                alert('Please select a vault first');
                return;
            }
            try {
                const fileName = await this.generateUniqueNotePath(this.state.currentVaultId);
                await this.api.createFile(this.state.currentVaultId, fileName, '');
                await this.loadFileTree();
                await this.openFile(fileName);
            } catch (error) {
                console.error('Failed to create unique note:', error);
                alert('Failed to create unique note: ' + error);
            }
        });

        // Random Note
        const randomNoteBtn = document.getElementById('random-note-btn');
        randomNoteBtn?.addEventListener('click', async () => {
            if (!this.state.currentVaultId) {
                alert('Please select a vault first');
                return;
            }

            try {
                const result = await this.api.getRandomNote(this.state.currentVaultId);
                if (result.path) {
                    this.openFile(result.path);
                }
            } catch (error) {
                console.error('Failed to get random note:', error);
                // Don't alert if it's just a 404/not found which might happen in empty vaults
                alert('No markdown files found in this vault');
            }
        });

        // Daily Note
        const dailyNoteBtn = document.getElementById('daily-note-btn');
        dailyNoteBtn?.addEventListener('click', async () => {
            if (!this.state.currentVaultId) {
                alert('Please select a vault first');
                return;
            }

            try {
                // Get today's date in YYYY-MM-DD format
                const today = new Date().toISOString().split('T')[0];
                const file = await this.api.getDailyNote(this.state.currentVaultId, today);
                this.openFile(file.path);

                // Refresh file tree in case file was created
                await this.loadFileTree();
            } catch (error) {
                console.error('Failed to get daily note:', error);
                alert('Failed to get daily note: ' + error);
            }
        });


        // Upload functionality
        this.setupUploadHandlers();
        this.setupDragAndDrop();

        // Bulk operations
        const selectAllBtn = document.getElementById('select-all-btn');
        selectAllBtn?.addEventListener('click', () => {
            this.selectAllFiles();
        });

        const selectNoneBtn = document.getElementById('select-none-btn');
        selectNoneBtn?.addEventListener('click', () => {
            this.deselectAllFiles();
        });

        const bulkDownloadBtn = document.getElementById('bulk-download-btn');
        bulkDownloadBtn?.addEventListener('click', async () => {
            await this.bulkDownload();
        });

        const bulkDeleteBtn = document.getElementById('bulk-delete-btn');
        bulkDeleteBtn?.addEventListener('click', async () => {
            await this.bulkDelete();
        });
    }

    setupUploadHandlers() {
        const uploadBtn = document.getElementById('upload-btn');
        const browseBtn = document.getElementById('browse-btn');
        const fileInput = document.getElementById('file-input') as HTMLInputElement;
        const folderInput = document.getElementById('folder-input') as HTMLInputElement;
        const uploadArea = document.getElementById('upload-area');
        const uploadPromptText = document.getElementById('upload-prompt-text');

        let currentUploadType: 'files' | 'folder' = 'files';

        // Upload type selector
        document.querySelectorAll('.upload-type-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const target = e.target as HTMLElement;
                const type = target.getAttribute('data-type') as 'files' | 'folder';

                // Update active state
                document.querySelectorAll('.upload-type-btn').forEach(b => b.classList.remove('active'));
                target.classList.add('active');

                currentUploadType = type;

                // Update UI text
                if (uploadPromptText) {
                    uploadPromptText.textContent = type === 'folder'
                        ? 'Drag and drop a folder here or click to browse'
                        : 'Drag and drop files here or click to browse';
                }
                if (browseBtn) {
                    browseBtn.textContent = type === 'folder' ? 'Browse Folder' : 'Browse Files';
                }
            });
        });

        uploadBtn?.addEventListener('click', () => {
            this.showModal('upload-modal');
        });

        browseBtn?.addEventListener('click', () => {
            if (currentUploadType === 'folder') {
                folderInput?.click();
            } else {
                fileInput?.click();
            }
        });

        uploadArea?.addEventListener('click', (e) => {
            if (e.target === uploadArea || (e.target as HTMLElement).closest('.upload-prompt')) {
                if (currentUploadType === 'folder') {
                    folderInput?.click();
                } else {
                    fileInput?.click();
                }
            }
        });

        fileInput?.addEventListener('change', (e) => {
            const files = (e.target as HTMLInputElement).files;
            if (files && files.length > 0) {
                this.displaySelectedFiles(files);
            }
        });

        folderInput?.addEventListener('change', (e) => {
            const files = (e.target as HTMLInputElement).files;
            if (files && files.length > 0) {
                this.displaySelectedFiles(files);
            }
        });

        // Upload area drag and drop
        uploadArea?.addEventListener('dragover', (e) => {
            e.preventDefault();
            uploadArea.classList.add('drag-over');
        });

        uploadArea?.addEventListener('dragleave', () => {
            uploadArea.classList.remove('drag-over');
        });

        uploadArea?.addEventListener('drop', (e) => {
            e.preventDefault();
            uploadArea.classList.remove('drag-over');
            const files = e.dataTransfer?.files;
            if (files && files.length > 0) {
                this.displaySelectedFiles(files);
            }
        });
    }

    setupDragAndDrop() {
        const dragOverlay = document.getElementById('drag-overlay');
        let dragCounter = 0;

        document.addEventListener('dragenter', (e) => {
            e.preventDefault();
            dragCounter++;
            if (this.state.currentVaultId) {
                dragOverlay?.classList.remove('hidden');
            }
        });

        document.addEventListener('dragleave', (e) => {
            e.preventDefault();
            dragCounter--;
            if (dragCounter === 0) {
                dragOverlay?.classList.add('hidden');
            }
        });

        document.addEventListener('dragover', (e) => {
            e.preventDefault();
        });

        document.addEventListener('drop', async (e) => {
            e.preventDefault();
            dragCounter = 0;
            dragOverlay?.classList.add('hidden');

            if (!this.state.currentVaultId) return;

            const files = e.dataTransfer?.files;
            if (files && files.length > 0) {
                await this.handleUpload(files);
            }
        });
    }

    // Editor-specific drag and drop for images/files
    setupEditorDropZone(container: HTMLElement, textarea?: HTMLTextAreaElement) {
        container.addEventListener('dragover', (e) => {
            e.preventDefault();
            e.stopPropagation();
            container.classList.add('editor-drag-over');
        });

        container.addEventListener('dragleave', (e) => {
            e.preventDefault();
            e.stopPropagation();
            // Only remove class if leaving the container entirely
            const relatedTarget = e.relatedTarget as HTMLElement;
            if (!container.contains(relatedTarget)) {
                container.classList.remove('editor-drag-over');
            }
        });

        container.addEventListener('drop', async (e) => {
            e.preventDefault();
            e.stopPropagation();
            container.classList.remove('editor-drag-over');

            const files = e.dataTransfer?.files;
            if (!files || files.length === 0) return;

            await this.handleEditorDrop(files, textarea);
        });
    }

    async handleEditorDrop(files: FileList, textarea?: HTMLTextAreaElement) {
        if (!this.state.currentVaultId || !this.state.activeTabId) return;

        const tab = this.state.getTab(this.state.activeTabId);
        if (!tab || tab.fileType !== 'markdown') return;

        // Upload files and get paths
        const uploadedPaths = await this.uploadFilesForEditor(files);
        if (uploadedPaths.length === 0) return;

        // Generate markdown for the uploaded files
        const markdownText = uploadedPaths
            .map(file => this.generateFileMarkdown(file.path, file.filename))
            .join('\n');

        // Insert into editor based on mode
        this.insertIntoEditor(markdownText, textarea);
    }

    async uploadFilesForEditor(files: FileList): Promise<Array<{ path: string, filename: string }>> {
        if (!this.state.currentVaultId) return [];

        const uploadedFiles: Array<{ path: string, filename: string }> = [];

        try {
            // Upload to attachments folder for organization
            const result = await this.api.uploadFiles(
                this.state.currentVaultId,
                files,
                'attachments'
            );

            if (result.uploaded && Array.isArray(result.uploaded)) {
                for (const file of result.uploaded) {
                    uploadedFiles.push({
                        path: file.path,
                        filename: file.filename
                    });
                }
            }

            // Refresh file tree to show new files
            this.loadFileTree();
        } catch (error) {
            console.error('Failed to upload files for editor:', error);
            alert('Failed to upload files: ' + error);
        }

        return uploadedFiles;
    }

    generateFileMarkdown(filePath: string, filename: string): string {
        const fileType = getFileType(filePath);

        // Use Obsidian-style wiki links for internal files
        switch (fileType) {
            case 'image':
                return `![[${filePath}]]`;
            case 'audio':
            case 'video':
                return `![[${filePath}]]`;
            case 'pdf':
                return `![[${filePath}]]`;
            default:
                return `[[${filePath}]]`;
        }
    }

    getCreationBasePath(): string {
        // Prefer the first selected item; if it has a parent, use that, otherwise root
        const selected = Array.from(this.state.selectedFiles || [])[0];
        if (!selected) return '';

        // If selection ends with slash (directory) use it, otherwise use parent path
        if (selected.endsWith('/')) {
            return selected.slice(0, -1);
        }
        const parent = selected.split('/').slice(0, -1).join('/');
        return parent;
    }

    async generateUniqueNotePath(vaultId: string): Promise<string> {
        // Get today's date in YYYYMMDD format
        const now = new Date();
        const year = now.getFullYear();
        const month = String(now.getMonth() + 1).padStart(2, '0');
        const day = String(now.getDate()).padStart(2, '0');
        const datePrefix = `${year}${month}${day}`;

        // Get file tree to find existing files with this prefix
        const tree = await this.api.getFileTree(vaultId);
        const flatFiles = this.flattenFileTree(tree);
        const existingNumbers: number[] = [];

        for (const file of flatFiles) {
            if (file.name.startsWith(datePrefix) && !file.is_directory) {
                // Extract the number part from YYYYMMDD#### format
                const numPart = file.name.replace(datePrefix, '').replace(/\.md$/, '');
                if (/^\d+$/.test(numPart)) {
                    existingNumbers.push(parseInt(numPart, 10));
                }
            }
        }

        // Find next available number (start from 0001)
        let nextNum = 1;
        while (existingNumbers.includes(nextNum)) {
            nextNum++;
        }
        const numPart = String(nextNum).padStart(4, '0');
        return `${datePrefix}${numPart}.md`;
    }

    flattenFileTree(nodes: FileNode[]): FileNode[] {
        const result: FileNode[] = [];
        for (const node of nodes) {
            result.push(node);
            if (node.children && Array.isArray(node.children)) {
                result.push(...this.flattenFileTree(node.children));
            }
        }
        return result;
    }

    insertIntoEditor(text: string, textarea?: HTMLTextAreaElement) {
        // For raw and side-by-side modes with textarea
        if (textarea) {
            this.insertTextIntoTextarea(text, textarea);
            return;
        }

        // Try to find active editor based on state
        const tab = this.state.activeTabId ? this.state.getTab(this.state.activeTabId) : null;
        if (!tab) return;

        const paneId = tab.pane;

        // Check for textarea in active pane
        const paneContainer = document.getElementById(paneId);
        if (paneContainer) {
            const activeTextarea = paneContainer.querySelector('textarea.editor-raw') as HTMLTextAreaElement;
            if (activeTextarea) {
                this.insertTextIntoTextarea(text, activeTextarea);
                return;
            }
        }

        // For formatted mode (CodeJar)
        const jar = this.paneJars.get(paneId);
        if (jar) {
            const currentContent = jar.toString();
            const newContent = currentContent + (currentContent.endsWith('\n') ? '' : '\n') + text + '\n';
            jar.updateCode(newContent);
            return;
        }

        // For WYSIWYG mode (Quill)
        const quill = this.paneQuills.get(paneId);
        if (quill) {
            const range = quill.getSelection(true);
            const index = range ? range.index : quill.getLength();

            // For images, insert as image embed; for other files, insert as link
            const lines = text.split('\n');
            for (const line of lines) {
                // Check if it's an image embed (![[...]])
                const imageMatch = line.match(/!\[\[([^\]]+)\]\]/);
                if (imageMatch && this.state.currentVaultId) {
                    const filePath = imageMatch[1];
                    const imageUrl = `/api/vaults/${this.state.currentVaultId}/raw/${filePath}`;
                    quill.insertEmbed(index, 'image', imageUrl, 'user');
                } else {
                    // Insert as text for other file types
                    quill.insertText(index, line + '\n', 'user');
                }
            }
            return;
        }
    }

    insertTextIntoTextarea(text: string, textarea: HTMLTextAreaElement) {
        const start = textarea.selectionStart;
        const end = textarea.selectionEnd;
        const before = textarea.value.substring(0, start);
        const after = textarea.value.substring(end);

        // Add newlines if needed
        const needsNewlineBefore = before.length > 0 && !before.endsWith('\n');
        const needsNewlineAfter = after.length > 0 && !after.startsWith('\n');

        const insertText = (needsNewlineBefore ? '\n' : '') + text + (needsNewlineAfter ? '\n' : '');

        textarea.value = before + insertText + after;
        textarea.selectionStart = textarea.selectionEnd = start + insertText.length;

        // Trigger input event to update tab content
        textarea.dispatchEvent(new Event('input', { bubbles: true }));
        textarea.focus();
    }

    displaySelectedFiles(files: FileList) {
        const uploadList = document.getElementById('upload-list');
        if (!uploadList) return;

        uploadList.innerHTML = '';

        const filesArray = Array.from(files);
        for (const file of filesArray) {
            const item = document.createElement('div');
            item.className = 'upload-item';
            item.innerHTML = `
                <span class="upload-item-name">${file.name}</span>
                <span class="upload-item-size">${this.formatFileSize(file.size)}</span>
                <button class="upload-item-remove" data-file="${file.name}">✕</button>
            `;
            uploadList.appendChild(item);
        }

        // Add upload button
        const uploadButton = document.createElement('button');
        uploadButton.className = 'btn btn-primary';
        uploadButton.textContent = `Upload ${filesArray.length} file(s)`;
        uploadButton.style.marginTop = '1rem';
        uploadButton.addEventListener('click', () => this.handleUpload(files));
        uploadList.appendChild(uploadButton);
    }

    async handleUpload(files: FileList) {
        if (!this.state.currentVaultId) {
            alert('Please select a vault first');
            return;
        }

        const progressContainer = document.getElementById('upload-progress');
        const progressBar = document.getElementById('progress-bar') as HTMLElement;
        const progressText = document.getElementById('progress-text');

        progressContainer?.classList.remove('hidden');

        try {
            await this.api.uploadFiles(
                this.state.currentVaultId,
                files,
                '',
                (loaded, total) => {
                    const percentage = Math.round((loaded / total) * 100);
                    if (progressBar) progressBar.style.width = `${percentage}%`;
                    if (progressText) progressText.textContent = `Uploading... ${percentage}%`;
                }
            );

            if (progressText) progressText.textContent = 'Upload complete!';
            setTimeout(() => {
                this.hideModal('upload-modal');
                progressContainer?.classList.add('hidden');
                this.loadFileTree();
            }, 1000);
        } catch (error) {
            console.error('Upload failed:', error);
            alert('Upload failed: ' + error);
            progressContainer?.classList.add('hidden');
        }
    }

    formatFileSize(bytes: number): string {
        if (bytes === 0) return '0 Bytes';
        const k = 1024;
        const sizes = ['Bytes', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
    }

    togglePropertiesPanel() {
        const panel = document.getElementById('properties-panel');
        if (!panel) return;

        if (panel.classList.contains('hidden')) {
            this.showPropertiesPanel();
        } else {
            this.hidePropertiesPanel();
        }
    }

    showPropertiesPanel() {
        const panel = document.getElementById('properties-panel');
        if (!panel) return;

        panel.classList.remove('hidden');
        this.renderProperties();
    }

    hidePropertiesPanel() {
        const panel = document.getElementById('properties-panel');
        panel?.classList.add('hidden');
    }

    renderProperties() {
        const content = document.getElementById('properties-content');
        if (!content || !this.state.activeTabId) return;

        const tab = this.state.getTab(this.state.activeTabId);
        if (!tab || tab.fileType !== 'markdown') {
            content.innerHTML = '<div class="empty-state"><p>No properties available for this file</p></div>';
            return;
        }

        const frontmatter = tab.frontmatter || {};
        content.innerHTML = '';

        // Render each property
        for (const [key, value] of Object.entries(frontmatter)) {
            const propertyItem = this.createPropertyItem(key, value);
            content.appendChild(propertyItem);
        }

        if (Object.keys(frontmatter).length === 0) {
            content.innerHTML = '<div class="empty-state"><p>No properties defined</p></div>';
        }
    }

    createPropertyItem(key: string, value: any): HTMLElement {
        const item = document.createElement('div');
        item.className = 'property-item';
        item.dataset.key = key;

        const valueType = Array.isArray(value) ? 'array' : typeof value;
        let valueStr = '';

        if (Array.isArray(value)) {
            valueStr = value.join(', ');
        } else if (typeof value === 'object' && value !== null) {
            valueStr = JSON.stringify(value);
        } else {
            valueStr = String(value);
        }

        item.innerHTML = `
            <div class="property-item-header">
                <input type="text" class="property-key" value="${key}" placeholder="Property name">
                <button class="property-remove-btn">Remove</button>
            </div>
            <div class="property-type-selector">
                <select class="property-type">
                    <option value="string" ${valueType === 'string' ? 'selected' : ''}>Text</option>
                    <option value="array" ${valueType === 'array' ? 'selected' : ''}>List</option>
                    <option value="number" ${valueType === 'number' ? 'selected' : ''}>Number</option>
                    <option value="boolean" ${valueType === 'boolean' ? 'selected' : ''}>Boolean</option>
                </select>
            </div>
            <textarea class="property-value" placeholder="Value">${valueStr}</textarea>
        `;

        // Add remove button handler
        const removeBtn = item.querySelector('.property-remove-btn');
        removeBtn?.addEventListener('click', () => {
            item.remove();
            if (document.querySelectorAll('.property-item').length === 0) {
                const content = document.getElementById('properties-content');
                if (content) {
                    content.innerHTML = '<div class="empty-state"><p>No properties defined</p></div>';
                }
            }
        });

        return item;
    }

    addProperty() {
        const content = document.getElementById('properties-content');
        if (!content) return;

        // Remove empty state if present
        const emptyState = content.querySelector('.empty-state');
        if (emptyState) {
            emptyState.remove();
        }

        const propertyItem = this.createPropertyItem('', '');
        content.appendChild(propertyItem);

        // Focus the key input
        const keyInput = propertyItem.querySelector('.property-key') as HTMLInputElement;
        keyInput?.focus();
    }

    async saveProperties() {
        if (!this.state.activeTabId || !this.state.currentVaultId) {
            alert('No file is currently open');
            return;
        }

        const tab = this.state.getTab(this.state.activeTabId);
        if (!tab || tab.fileType !== 'markdown') {
            alert('Properties can only be saved for markdown files');
            return;
        }

        // Collect properties from UI
        const properties: any = {};
        const propertyItems = document.querySelectorAll('.property-item');

        for (const item of Array.from(propertyItems)) {
            const keyInput = item.querySelector('.property-key') as HTMLInputElement;
            const valueTextarea = item.querySelector('.property-value') as HTMLTextAreaElement;
            const typeSelect = item.querySelector('.property-type') as HTMLSelectElement;

            const key = keyInput.value.trim();
            const valueStr = valueTextarea.value.trim();
            const type = typeSelect.value;

            if (!key) continue; // Skip empty keys

            let value: any;
            switch (type) {
                case 'array':
                    value = valueStr.split(',').map(v => v.trim()).filter(v => v);
                    break;
                case 'number':
                    value = parseFloat(valueStr) || 0;
                    break;
                case 'boolean':
                    value = valueStr.toLowerCase() === 'true';
                    break;
                default:
                    value = valueStr;
            }

            properties[key] = value;
        }

        // Update tab frontmatter
        tab.frontmatter = Object.keys(properties).length > 0 ? properties : undefined;
        tab.isDirty = true;

        // Save the file
        try {
            await this.api.writeFile(
                this.state.currentVaultId,
                tab.filePath,
                tab.content,
                tab.modified,
                tab.frontmatter
            );
            tab.isDirty = false;
            this.renderTabs();
            alert('Properties saved successfully');
        } catch (error) {
            console.error('Failed to save properties:', error);
            alert('Failed to save properties: ' + error);
        }
    }

    async performSearch(query: string) {
        if (!this.state.currentVaultId) return;

        try {
            const results = await this.api.search(this.state.currentVaultId, query);
            this.renderSearchResults(results);
            this.showModal('search-modal');
        } catch (error) {
            console.error('Search failed:', error);
        }
    }

    renderSearchResults(results: SearchResult[]) {
        const container = document.getElementById('search-results');
        if (!container) return;

        if (results.length === 0) {
            container.innerHTML = '<p>No results found</p>';
            return;
        }

        container.innerHTML = '';
        for (const result of results) {
            const item = document.createElement('div');
            item.className = 'search-result-item';
            item.innerHTML = `
                <div class="search-result-title">${result.title}</div>
                <div class="search-result-path">${result.path}</div>
                <div class="search-result-matches">
                    ${result.matches.map(m => `
                        <div class="search-match">
                            <span class="line-number">${m.line_number}:</span>
                            ${this.highlightMatch(m.line_text, m.match_start, m.match_end)}
                        </div>
                    `).join('')}
                </div>
            `;

            item.addEventListener('click', () => {
                this.openFile(result.path);
                this.hideModal('search-modal');
            });

            container.appendChild(item);
        }
    }

    highlightMatch(text: string, start: number, end: number): string {
        return text.substring(0, start) +
            '<mark>' + text.substring(start, end) + '</mark>' +
            text.substring(end);
    }

    escapeHtml(unsafe: string): string {
        return unsafe
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#039;");
    }

    showModal(modalId: string) {
        const modal = document.getElementById(modalId);
        modal?.classList.remove('hidden');
    }

    hideModal(modalId: string) {
        const modal = document.getElementById(modalId);
        modal?.classList.add('hidden');
    }

    setupWebSocket() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const ws = new WebSocket(`${protocol}//${window.location.host}/api/ws`);

        ws.onmessage = async (event) => {
            try {
                const changeEvent = JSON.parse(event.data);
                console.log('File change event:', changeEvent);

                // Only process events for current vault
                if (!this.state.currentVaultId || changeEvent.vault_id !== this.state.currentVaultId) return;

                const { event_type, path } = changeEvent;

                // Handle file tree updates
                if (event_type === 'created' || event_type === 'deleted' || (typeof event_type === 'object' && 'renamed' in event_type) || event_type === 'Created' || event_type === 'Deleted' || (typeof event_type === 'object' && 'Renamed' in event_type)) {
                    const fileTree = document.getElementById('file-tree');
                    if (fileTree && fileTree.hasAttribute('hx-get') && typeof htmx !== 'undefined') {
                        htmx.trigger(fileTree, 'load');
                    } else {
                        await this.loadFileTree();
                    }
                }

                // Handle open tabs
                for (const [tabId, tab] of this.state.openTabs.entries()) {
                    if (tab.filePath === path) {
                        if (event_type === 'Modified') {
                            // Reload content if not dirty, otherwise notify
                            if (!tab.isDirty) {
                                // If it's the active tab, we might want to refresh immediately or show a toast
                                // For now, let's just reload content in background if we can, or just re-fetch on focus?
                                // Simple approach: reload content
                                const fileData = await this.api.readFile(this.state.currentVaultId, path);
                                tab.content = fileData.content;
                                tab.modified = fileData.modified;
                                if (tab.id === this.state.activeTabId) {
                                    this.renderEditor(); // Refresh editor
                                }
                            } else {
                                // Notify user of conflict?
                                console.warn('External modification on dirty file:', path);
                            }
                        } else if (event_type === 'Deleted') {
                            // Close tab or warn?
                            // Let's close it for now or show it as deleted
                            alert(`File ${path} was deleted externally.`);
                            this.state.removeTab(tabId);
                            if (this.state.activeTabId === tabId) {
                                this.state.activeTabId = null;
                            }
                            this.renderTabs();
                            this.renderEditor();
                        }
                    }

                    // Handle Renamed
                    if (typeof event_type === 'object' && 'Renamed' in event_type) {
                        const renamedEvent = event_type as any;
                        if (renamedEvent.from === tab.filePath) {
                            tab.filePath = renamedEvent.to;
                            tab.fileName = renamedEvent.to.split('/').pop() || renamedEvent.to;
                            this.renderTabs();
                        }
                    }
                }

            } catch (error) {
                console.error('Failed to parse WebSocket message:', error);
            }
        };

        ws.onopen = () => {
            console.log('WebSocket connected');
            this.state.wsReconnectAttempts = 0;
            this.updateConnectionStatus('connected');
        };

        ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.updateConnectionStatus('error');
        };

        ws.onclose = (event) => {
            console.log('WebSocket closed:', event.code, event.reason);
            this.state.ws = null;
            this.updateConnectionStatus('disconnected');

            // Clear any existing reconnect timeout
            if (this.state.wsReconnectTimeout) {
                clearTimeout(this.state.wsReconnectTimeout);
            }

            // Exponential backoff: 1s, 2s, 4s, 8s, 16s, 30s (max)
            this.state.wsReconnectAttempts++;
            const delay = Math.min(
                1000 * Math.pow(2, this.state.wsReconnectAttempts - 1),
                this.state.wsMaxReconnectDelay
            );

            console.log(`Reconnecting in ${delay / 1000}s (attempt ${this.state.wsReconnectAttempts})...`);
            this.updateConnectionStatus('reconnecting', delay);

            this.state.wsReconnectTimeout = window.setTimeout(() => {
                this.setupWebSocket();
            }, delay);
        };

        this.state.ws = ws;
    }

    updateConnectionStatus(status: 'connected' | 'disconnected' | 'reconnecting' | 'error', delay?: number) {
        // Update UI to show connection status
        // This could be a status indicator in the header
        const statusElement = document.getElementById('connection-status');
        if (!statusElement) return;

        statusElement.className = `connection-status connection-${status}`;

        switch (status) {
            case 'connected':
                statusElement.textContent = '●';
                statusElement.title = 'Connected';
                statusElement.style.color = '#4ade80'; // green
                break;
            case 'disconnected':
                statusElement.textContent = '●';
                statusElement.title = 'Disconnected';
                statusElement.style.color = '#ef4444'; // red
                break;
            case 'reconnecting':
                statusElement.textContent = '●';
                statusElement.title = `Reconnecting${delay ? ` in ${delay / 1000}s` : '...'}`;
                statusElement.style.color = '#fbbf24'; // yellow
                break;
            case 'error':
                statusElement.textContent = '●';
                statusElement.title = 'Connection error';
                statusElement.style.color = '#f87171'; // light red
                break;
        }
    }

    setupQuickSwitcher() {
        const modal = document.getElementById('quick-switcher-modal');
        const input = document.getElementById('quick-switcher-input') as HTMLInputElement;
        const resultsContainer = document.getElementById('quick-switcher-results');

        // Global keyboard shortcut (Ctrl+O or Cmd+O not working well in browser, usually opens files)
        // Using Ctrl+K or Cmd+K
        document.addEventListener('keydown', (e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
                e.preventDefault();
                this.showModal('quick-switcher-modal');
                input?.focus();
                input.value = '';
                if (resultsContainer) resultsContainer.innerHTML = '';
                // Pre-load recent files or all files could be implemented here
                this.performQuickSwitcherSearch('');
            }
        });

        // Close on Escape
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape' && !modal?.classList.contains('hidden')) {
                this.hideModal('quick-switcher-modal');
            }
        });

        // Search input handler
        input?.addEventListener('input', (e) => {
            const query = (e.target as HTMLInputElement).value;

            if (this.state.quickSwitcherDebounce) {
                clearTimeout(this.state.quickSwitcherDebounce);
            }

            this.state.quickSwitcherDebounce = window.setTimeout(() => {
                this.performQuickSwitcherSearch(query);
            }, 200);
        });

        // Keyboard navigation in list
        input?.addEventListener('keydown', (e) => {
            if (!resultsContainer) return;

            const items = resultsContainer.querySelectorAll('.search-result-item');
            const activeItem = resultsContainer.querySelector('.search-result-item.active');
            let index = Array.from(items).indexOf(activeItem as Element);

            if (e.key === 'ArrowDown') {
                e.preventDefault();
                index = index < items.length - 1 ? index + 1 : 0;
                this.highlightQuickSwitcherItem(items, index);
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                index = index > 0 ? index - 1 : items.length - 1;
                this.highlightQuickSwitcherItem(items, index);
            } else if (e.key === 'Enter') {
                e.preventDefault();
                if (activeItem) {
                    (activeItem as HTMLElement).click();
                } else if (items.length > 0) {
                    // Default to first item if none active
                    (items[0] as HTMLElement).click();
                }
            }
        });
    }

    highlightQuickSwitcherItem(items: NodeListOf<Element>, index: number) {
        items.forEach(item => item.classList.remove('active'));
        if (items[index]) {
            items[index].classList.add('active');
            (items[index] as HTMLElement).scrollIntoView({ block: 'nearest' });
        }
    }

    async performQuickSwitcherSearch(query: string) {
        if (!this.state.currentVaultId) return;

        try {
            // Reuse the search API but with a different limit or params if needed
            // For quick switcher, we mostly care about file paths/names
            // If query is empty, maybe show recent files? For now just show nothing or all files
            let results: SearchResult[] = [];

            if (query.trim() === '') {
                // Show recent files
                results = this.state.recentFiles.map(path => ({
                    title: path.split('/').pop() || path,
                    path: path,
                    score: 0,
                    matches: []
                }));
            } else {
                results = await this.api.search(this.state.currentVaultId, query, 20);
            }

            this.renderQuickSwitcherResults(results);
        } catch (error) {
            console.error('Quick switcher search failed:', error);
        }
    }

    renderQuickSwitcherResults(results: SearchResult[]) {
        const container = document.getElementById('quick-switcher-results');
        if (!container) return;

        if (results.length === 0) {
            container.innerHTML = '<div class="empty-state"><p>No matching files</p></div>';
            return;
        }

        container.innerHTML = '';
        results.forEach((result, index) => {
            const item = document.createElement('div');
            item.className = 'search-result-item'; // Reusing search styles for now
            if (index === 0) item.classList.add('active');

            item.innerHTML = `
            <div class="search-result-title">${result.title}</div>
            <div class="search-result-path">${result.path}</div>
        `;

            item.addEventListener('click', () => {
                this.openFile(result.path);
                this.hideModal('quick-switcher-modal');
            });

            // Hover effect
            item.addEventListener('mouseenter', () => {
                container.querySelectorAll('.search-result-item').forEach(i => i.classList.remove('active'));
                item.classList.add('active');
            });

            container.appendChild(item);
        });
    }
    setupTemplates() {
        const btn = document.getElementById('insert-template-btn');
        btn?.addEventListener('click', () => {
            if (!this.state.currentVaultId) {
                alert('Please select a vault first');
                return;
            }
            if (!this.state.activeTabId) {
                alert('Please open a file first');
                return;
            }
            // Check if file is editable
            const tab = this.state.getTab(this.state.activeTabId);
            if (tab?.fileType !== 'markdown') {
                alert('Templates can only be inserted into markdown files');
                return;
            }

            this.showModal('insert-template-modal');
            this.loadTemplates();
        });
    }

    async loadTemplates() {
        if (!this.state.currentVaultId) return;
        const listContainer = document.getElementById('template-list');
        if (!listContainer) return;

        listContainer.innerHTML = '<p>Loading templates...</p>';

        try {
            // Find "Templates" folder
            // We'll search for a directory named "Templates" (case insensitive)
            // Implementation detail: we fetch file tree and look for it
            const tree = await this.api.getFileTree(this.state.currentVaultId);

            // Helper to find folder
            const findTemplatesFolder = (nodes: FileNode[]): FileNode | null => {
                for (const node of nodes) {
                    if (node.is_directory && node.name.toLowerCase() === 'templates') {
                        return node;
                    }
                    if (node.children) {
                        const found = findTemplatesFolder(node.children);
                        if (found) return found;
                    }
                }
                return null;
            };

            const templatesFolder = findTemplatesFolder(tree);

            if (!templatesFolder || !templatesFolder.children || templatesFolder.children.length === 0) {
                listContainer.innerHTML = `
                <div class="empty-state">
                    <p>No templates found.</p>
                    <small>Create a folder named "Templates" and add some markdown files.</small>
                    <button id="create-default-templates-btn" class="btn btn-primary" style="margin-top: 10px;">Create Default Templates</button>
                </div>
            `;

                // Add listener
                document.getElementById('create-default-templates-btn')?.addEventListener('click', () => {
                    this.createDefaultTemplates();
                });
                return;
            }

            this.renderTemplates(templatesFolder.children);

        } catch (error) {
            console.error('Failed to load templates:', error);
            listContainer.innerHTML = `<p class="error">Failed to load templates: ${error}</p>`;
        }
    }

    renderTemplates(nodes: FileNode[]) {
        const listContainer = document.getElementById('template-list');
        if (!listContainer) return;

        listContainer.innerHTML = '';

        // Filter for markdown files only
        const templateFiles = nodes.filter(n => !n.is_directory && n.name.endsWith('.md'));

        if (templateFiles.length === 0) {
            listContainer.innerHTML = '<p>No template files found in Templates folder.</p>';
            return;
        }

        templateFiles.forEach(node => {
            const item = document.createElement('div');
            item.className = 'template-item';
            item.innerHTML = `
                <span class="file-icon">📄</span>
                <span class="file-name">${node.name}</span>
            `;

            item.addEventListener('click', () => {
                this.insertTemplate(node.path);
                this.hideModal('insert-template-modal');
            });

            listContainer.appendChild(item);
        });
    }

    async insertTemplate(templatePath: string) {
        if (!this.state.currentVaultId || !this.state.activeTabId) return;

        try {
            const templateContent = await this.api.readFile(this.state.currentVaultId, templatePath);
            const processedContent = this.applyTemplateVariables(templateContent.content);

            // Insert into active editor
            const pane1 = document.getElementById('pane-1');
            const textarea = pane1?.querySelector('textarea');

            if (textarea) {
                // Insert at cursor position
                const startPos = textarea.selectionStart;
                const endPos = textarea.selectionEnd;
                const currentVal = textarea.value;

                const newVal = currentVal.substring(0, startPos) + processedContent + currentVal.substring(endPos);

                textarea.value = newVal;
                // Update cursor position
                const newCursorPos = startPos + processedContent.length;
                textarea.setSelectionRange(newCursorPos, newCursorPos);
                textarea.focus();

                // Trigger input event to update tab state
                textarea.dispatchEvent(new Event('input'));
            } else {
                console.warn('Could not find textarea to insert template');
                alert('Could not insert template: Editor not found or not in raw mode.');
            }

        } catch (error) {
            console.error('Failed to insert template:', error);
            alert('Failed to insert template: ' + error);
        }
    }

    applyTemplateVariables(content: string): string {
        const now = new Date();

        // Format: YYYY-MM-DD
        const dateStr = now.toISOString().split('T')[0];

        // Format: HH:mm
        const timeStr = now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });

        let processed = content;
        processed = processed.replace(/{{date}}/g, dateStr);
        processed = processed.replace(/{{time}}/g, timeStr);
        processed = processed.replace(/{{datetime}}/g, `${dateStr} ${timeStr}`);

        // {{title}} - Current file name
        const tab = this.state.getTab(this.state.activeTabId!);
        if (tab) {
            const title = tab.fileName.replace(/\.md$/, '');
            processed = processed.replace(/{{title}}/g, title);
        }

        return processed;
    }

    async loadPlugins() {
        try {
            const response = await fetch('/api/plugins');
            if (!response.ok) throw new Error('Failed to load plugins');

            const data = await response.json();
            this.renderInstalledPlugins(data.plugins || []);
        } catch (error) {
            console.error('Failed to load plugins:', error);
            const statsText = document.getElementById('plugin-stats-text');
            if (statsText) {
                statsText.textContent = 'Failed to load plugins';
            }
        }
    }

    renderInstalledPlugins(plugins: any[]) {
        const container = document.getElementById('installed-plugins-list');
        const statsText = document.getElementById('plugin-stats-text');

        if (!container) return;

        if (statsText) {
            statsText.textContent = `${plugins.length} plugin${plugins.length !== 1 ? 's' : ''} installed`;
        }

        if (plugins.length === 0) {
            container.innerHTML = '<div class="empty-state"><p>No plugins installed</p></div>';
            return;
        }

        container.innerHTML = '';

        plugins.forEach((plugin: any) => {
            const state = plugin.state || 'Unloaded';
            const stateClass = `plugin-state-${state.toLowerCase()}`;
            const isEnabled = plugin.enabled !== false;

            const item = document.createElement('div');
            item.className = 'plugin-item';
            item.setAttribute('data-plugin-id', plugin.manifest.id);

            item.innerHTML = `
                <div class="plugin-item-icon">🧩</div>
                <div class="plugin-item-content">
                    <div class="plugin-item-header">
                        <span class="plugin-item-name">${this.escapeHtml(plugin.manifest.name)}</span>
                        <span class="plugin-item-version">v${this.escapeHtml(plugin.manifest.version)}</span>
                    </div>
                    <div class="plugin-item-description">${this.escapeHtml(plugin.manifest.description)}</div>
                    <div class="plugin-item-meta">
                        <span>By ${this.escapeHtml(plugin.manifest.author || 'Unknown')}</span>
                    </div>
                </div>
                <div class="plugin-item-actions">
                    <span class="plugin-state-badge ${stateClass}">${state}</span>
                    <button class="plugin-toggle-btn ${isEnabled ? 'btn-disable' : 'btn-enable'}" data-plugin-id="${this.escapeHtml(plugin.manifest.id)}">${isEnabled ? 'Disable' : 'Enable'}</button>
                    <button class="plugin-settings-btn" data-plugin-id="${this.escapeHtml(plugin.manifest.id)}" title="Settings">⚙️</button>
                </div>
            `;

            container.appendChild(item);
        });

        // Add event listeners for toggle buttons
        container.querySelectorAll('.plugin-toggle-btn').forEach(btn => {
            btn.addEventListener('click', async (e) => {
                const target = e.target as HTMLElement;
                const pluginId = target.getAttribute('data-plugin-id');
                if (pluginId) {
                    await this.togglePlugin(pluginId, target.classList.contains('btn-enable'));
                }
            });
        });

        // Add event listeners for settings buttons
        container.querySelectorAll('.plugin-settings-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const target = e.target as HTMLElement;
                const pluginId = target.getAttribute('data-plugin-id');
                if (pluginId) {
                    this.showPluginSettings(pluginId);
                }
            });
        });
    }

    async togglePlugin(pluginId: string, shouldEnable: boolean) {
        try {
            const response = await fetch(`/api/plugins/${pluginId}/toggle`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({ enabled: shouldEnable })
            });

            if (!response.ok) throw new Error('Failed to toggle plugin');

            // Reload plugins to update UI
            await this.loadPlugins();
        } catch (error) {
            console.error('Failed to toggle plugin:', error);
            alert('Failed to toggle plugin. Please try again.');
        }
    }

    showPluginSettings(pluginId: string) {
        // Switch to Settings tab
        this.switchPluginTab('settings');

        // Show plugin settings
        const settingsContainer = document.getElementById('plugin-tab-settings');
        if (settingsContainer) {
            settingsContainer.innerHTML = `
                <div class="plugin-settings-container">
                    <h3>Settings for ${this.escapeHtml(pluginId)}</h3>
                    <p>Plugin settings configuration coming soon...</p>
                    <button class="btn-secondary" id="back-to-installed">← Back to Installed</button>
                </div>
            `;

            const backBtn = document.getElementById('back-to-installed');
            backBtn?.addEventListener('click', () => {
                this.switchPluginTab('installed');
            });
        }
    }

    switchPluginTab(tabName: string) {
        // Update tab buttons
        document.querySelectorAll('.plugin-tab-btn').forEach(btn => {
            btn.classList.remove('active');
            if (btn.getAttribute('data-tab') === tabName) {
                btn.classList.add('active');
            }
        });

        // Update tab content
        document.querySelectorAll('.plugin-tab-content').forEach(content => {
            content.classList.add('hidden');
        });

        const targetTab = document.getElementById(`plugin-tab-${tabName}`);
        if (targetTab) {
            targetTab.classList.remove('hidden');
        }
    }

    setupPluginManager() {
        // Add event listeners for plugin tab buttons
        document.querySelectorAll('.plugin-tab-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const target = e.target as HTMLElement;
                const tabName = target.getAttribute('data-tab');
                if (tabName) {
                    this.switchPluginTab(tabName);
                }
            });
        });
    }

    async createDefaultTemplates() {
        if (!this.state.currentVaultId) return;

        try {
            // Create Templates directory
            // We use a try/catch here in case it already exists (api might error or succeed depending on implementation)
            try {
                await this.api.createDirectory(this.state.currentVaultId, 'Templates');
            } catch (e: any) {
                // Ignore if it already exists or handle specifically
                if (!e.toString().includes('already exists')) {
                    // console.warn('Directory creation warning:', e);
                }
            }

            // Create Daily Note Template
            await this.api.createFile(
                this.state.currentVaultId,
                'Templates/Daily Note.md',
                '# {{date}}\n\n## Tasks\n- [ ] \n\n## Notes\n'
            );

            // Create Meeting Note Template
            await this.api.createFile(
                this.state.currentVaultId,
                'Templates/Meeting Note.md',
                '# {{title}}\nDate: {{datetime}}\n\n## Attendees\n\n## Agenda\n\n## Notes\n'
            );

            // Reload templates
            await this.loadTemplates();
            alert('Default templates created successfully.');

        } catch (error) {
            console.error('Failed to create default templates:', error);
            alert('Failed to create default templates: ' + error);
        }
    }

    setupConflictResolution() {
        const keepYoursBtn = document.getElementById('conflict-keep-yours');
        const useServerBtn = document.getElementById('conflict-use-server');
        const viewBothBtn = document.getElementById('conflict-view-both');
        const cancelBtn = document.getElementById('conflict-cancel');

        keepYoursBtn?.addEventListener('click', () => {
            if (!this.state.conflictData) return;
            console.log('User chose to keep their version');
            this.hideModal('conflict-modal');
            alert('Your changes will be saved (save functionality to be implemented)');
        });

        useServerBtn?.addEventListener('click', async () => {
            if (!this.state.conflictData || !this.state.currentVaultId) return;

            try {
                const fileData = await this.api.readFile(
                    this.state.currentVaultId,
                    this.state.conflictData.filePath
                );

                for (const [_, tab] of this.state.openTabs.entries()) {
                    if (tab.filePath === this.state.conflictData.filePath) {
                        tab.content = fileData.content;
                        tab.modified = fileData.modified;
                        tab.isDirty = false;
                        if (tab.id === this.state.activeTabId) {
                            this.renderEditor();
                        }
                        break;
                    }
                }

                this.hideModal('conflict-modal');
                this.state.conflictData = null;
            } catch (error) {
                console.error('Failed to load server version:', error);
                alert('Failed to load server version: ' + error);
            }
        });

        viewBothBtn?.addEventListener('click', () => {
            if (!this.state.conflictData) return;
            console.log('View both versions');
            alert('Side-by-side comparison view (to be implemented)');
        });

        cancelBtn?.addEventListener('click', () => {
            this.hideModal('conflict-modal');
            this.state.conflictData = null;
        });
    }

    showConflictResolution(filePath: string, yourVersion: string, serverVersion: string) {
        this.state.conflictData = { filePath, yourVersion, serverVersion };

        const fileNameEl = document.getElementById('conflict-file-name');
        const yourVersionEl = document.getElementById('conflict-your-version');
        const serverVersionEl = document.getElementById('conflict-server-version');

        if (fileNameEl) fileNameEl.textContent = filePath;
        if (yourVersionEl) yourVersionEl.textContent = yourVersion.substring(0, 500) + (yourVersion.length > 500 ? '...' : '');
        if (serverVersionEl) serverVersionEl.textContent = serverVersion.substring(0, 500) + (serverVersion.length > 500 ? '...' : '');

        this.showModal('conflict-modal');
    }

    setupUndoRedo() {
        // Keyboard shortcuts for undo/redo
        document.addEventListener('keydown', (e) => {
            // Check if we're in an input/textarea that handles its own undo
            const target = e.target as HTMLElement;
            const isInEditor = target.id.startsWith('editor-textarea-') ||
                target.id.startsWith('editor-formatted-') ||
                target.closest('.editor-wysiwyg') !== null;

            // Only intercept if we're in one of our editors
            if (!isInEditor) return;

            // Undo: Ctrl+Z / Cmd+Z
            if ((e.ctrlKey || e.metaKey) && e.key === 'z' && !e.shiftKey) {
                e.preventDefault();
                this.undo();
            }

            // Redo: Ctrl+Y / Cmd+Y or Ctrl+Shift+Z / Cmd+Shift+Z
            if ((e.ctrlKey || e.metaKey) && (e.key === 'y' || (e.key === 'z' && e.shiftKey))) {
                e.preventDefault();
                this.redo();
            }
        });

        // Button handlers are now attached in renderPanesStructure
    }

    setupInsertHelpers() {
        // Button handlers are now attached in renderPanesStructure because they are per-pane

        // Insert Link Form
        const insertLinkForm = document.getElementById('insert-link-form') as HTMLFormElement;
        insertLinkForm?.addEventListener('submit', (e) => {
            e.preventDefault();
            this.handleInsertLink();
        });

        // Link type toggle - update placeholder
        const linkTypeRadios = document.querySelectorAll('input[name="link-type"]');
        linkTypeRadios.forEach(radio => {
            radio.addEventListener('change', (e) => {
                const target = e.target as HTMLInputElement;
                const urlInput = document.getElementById('link-url') as HTMLInputElement;
                if (target.value === 'internal') {
                    urlInput.placeholder = 'Note Name';
                } else {
                    urlInput.placeholder = 'https://...';
                }
            });
        });

        // Image tab switching
        const imageTabs = document.querySelectorAll('.image-insert-tabs .tab-btn');
        imageTabs.forEach(tab => {
            tab.addEventListener('click', (e) => {
                const target = e.target as HTMLElement;
                const tabName = target.getAttribute('data-tab');
                if (!tabName) return;

                // Update active tab
                imageTabs.forEach(t => t.classList.remove('active'));
                target.classList.add('active');

                // Show corresponding content
                document.querySelectorAll('.image-tab-content').forEach(content => {
                    content.classList.add('hidden');
                });
                document.getElementById(`image-tab-${tabName}`)?.classList.remove('hidden');

                // Load vault images if switching to vault tab
                if (tabName === 'vault') {
                    this.loadVaultImages();
                }
            });
        });

        // Image upload handling
        const imageUploadArea = document.getElementById('image-upload-area');
        const imageFileInput = document.getElementById('image-file-input') as HTMLInputElement;
        const imageBrowseBtn = document.getElementById('image-browse-btn');

        imageBrowseBtn?.addEventListener('click', () => {
            imageFileInput?.click();
        });

        imageUploadArea?.addEventListener('click', (e) => {
            if (e.target === imageUploadArea || (e.target as HTMLElement).closest('.upload-prompt')) {
                imageFileInput?.click();
            }
        });

        imageFileInput?.addEventListener('change', (e) => {
            const files = (e.target as HTMLInputElement).files;
            if (files && files.length > 0) {
                this.previewSelectedImage(files[0]);
            }
        });

        // Drag and drop for image upload
        imageUploadArea?.addEventListener('dragover', (e) => {
            e.preventDefault();
            imageUploadArea.classList.add('drag-over');
        });

        imageUploadArea?.addEventListener('dragleave', () => {
            imageUploadArea.classList.remove('drag-over');
        });

        imageUploadArea?.addEventListener('drop', (e) => {
            e.preventDefault();
            imageUploadArea.classList.remove('drag-over');
            const files = e.dataTransfer?.files;
            if (files && files.length > 0 && files[0].type.startsWith('image/')) {
                this.previewSelectedImage(files[0]);
            }
        });

        // URL image preview
        const imageUrlInput = document.getElementById('image-url') as HTMLInputElement;
        imageUrlInput?.addEventListener('input', debounce(() => {
            const url = imageUrlInput.value.trim();
            if (url) {
                this.previewUrlImage(url);
            } else {
                document.getElementById('url-image-preview-container')?.classList.add('hidden');
            }
        }, 500));

        // Vault image search
        const vaultImageSearch = document.getElementById('vault-image-search') as HTMLInputElement;
        vaultImageSearch?.addEventListener('input', debounce(() => {
            this.loadVaultImages(vaultImageSearch.value);
        }, 300));

        // Insert Image Submit
        const insertImageSubmit = document.getElementById('insert-image-submit');
        insertImageSubmit?.addEventListener('click', () => {
            this.handleInsertImage();
        });

        // Keyboard shortcut for insert link (Ctrl+Shift+K to avoid conflict with quick switcher)
        document.addEventListener('keydown', (e) => {
            if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'k') {
                e.preventDefault();
                const tab = this.state.activeTabId ? this.state.getTab(this.state.activeTabId) : null;
                if (tab?.fileType === 'markdown') {
                    this.showInsertLinkModal();
                }
            }
        });
    }

    private selectedImageFile: File | null = null;
    private selectedVaultImage: string | null = null;

    showInsertLinkModal() {
        if (!this.state.activeTabId) {
            alert('Please open a file first');
            return;
        }

        const tab = this.state.getTab(this.state.activeTabId);
        if (!tab || tab.fileType !== 'markdown') {
            alert('Links can only be inserted into markdown files');
            return;
        }

        // Get selected text from editor to use as link text
        const selectedText = this.getSelectedText();

        // Reset form
        const linkTextInput = document.getElementById('link-text') as HTMLInputElement;
        const linkUrlInput = document.getElementById('link-url') as HTMLInputElement;
        const externalRadio = document.querySelector('input[name="link-type"][value="external"]') as HTMLInputElement;

        if (linkTextInput) linkTextInput.value = selectedText || '';
        if (linkUrlInput) {
            linkUrlInput.value = '';
            linkUrlInput.placeholder = 'https://...';
        }
        if (externalRadio) externalRadio.checked = true;

        this.showModal('insert-link-modal');
        linkUrlInput?.focus();
    }

    showInsertImageModal() {
        if (!this.state.activeTabId) {
            alert('Please open a file first');
            return;
        }

        const tab = this.state.getTab(this.state.activeTabId);
        if (!tab || tab.fileType !== 'markdown') {
            alert('Images can only be inserted into markdown files');
            return;
        }

        // Reset state
        this.selectedImageFile = null;
        this.selectedVaultImage = null;

        // Reset UI
        const imageFileInput = document.getElementById('image-file-input') as HTMLInputElement;
        const imageUrlInput = document.getElementById('image-url') as HTMLInputElement;
        const altTextInput = document.getElementById('image-alt-text') as HTMLInputElement;

        if (imageFileInput) imageFileInput.value = '';
        if (imageUrlInput) imageUrlInput.value = '';
        if (altTextInput) altTextInput.value = '';

        document.getElementById('image-preview-container')?.classList.add('hidden');
        document.getElementById('url-image-preview-container')?.classList.add('hidden');

        // Reset to upload tab
        document.querySelectorAll('.image-insert-tabs .tab-btn').forEach(t => t.classList.remove('active'));
        document.querySelector('.image-insert-tabs .tab-btn[data-tab="upload"]')?.classList.add('active');
        document.querySelectorAll('.image-tab-content').forEach(c => c.classList.add('hidden'));
        document.getElementById('image-tab-upload')?.classList.remove('hidden');

        this.showModal('insert-image-modal');
    }

    getSelectedText(): string {
        const pane1 = document.getElementById('pane-1');
        if (!pane1) return '';

        // Check textarea (raw or side-by-side mode)
        const textarea = pane1.querySelector('#editor-textarea') as HTMLTextAreaElement;
        if (textarea) {
            return textarea.value.substring(textarea.selectionStart, textarea.selectionEnd);
        }

        // Check CodeJar (formatted mode)
        if (this.currentJar) {
            const selection = window.getSelection();
            if (selection && selection.rangeCount > 0) {
                return selection.toString();
            }
        }

        // Check Quill (rendered mode)
        if (this.currentQuill) {
            const range = this.currentQuill.getSelection();
            if (range && range.length > 0) {
                return this.currentQuill.getText(range.index, range.length);
            }
        }

        return '';
    }

    handleInsertLink() {
        const linkTextInput = document.getElementById('link-text') as HTMLInputElement;
        const linkUrlInput = document.getElementById('link-url') as HTMLInputElement;
        const linkType = (document.querySelector('input[name="link-type"]:checked') as HTMLInputElement)?.value;

        const text = linkTextInput?.value.trim() || '';
        const url = linkUrlInput?.value.trim() || '';

        if (!url) {
            alert('Please enter a URL or note name');
            return;
        }

        let markdown: string;
        if (linkType === 'internal') {
            // Wiki-style internal link
            if (text && text !== url) {
                markdown = `[[${url}|${text}]]`;
            } else {
                markdown = `[[${url}]]`;
            }
        } else {
            // Standard markdown external link
            const displayText = text || url;
            markdown = `[${displayText}](${url})`;
        }

        this.insertTextAtCursor(markdown);
        this.hideModal('insert-link-modal');
    }

    previewSelectedImage(file: File) {
        this.selectedImageFile = file;

        const previewContainer = document.getElementById('image-preview-container');
        const previewImg = document.getElementById('image-preview') as HTMLImageElement;
        const previewName = document.getElementById('image-preview-name');

        if (previewContainer && previewImg && previewName) {
            const reader = new FileReader();
            reader.onload = (e) => {
                previewImg.src = e.target?.result as string;
                previewName.textContent = file.name;
                previewContainer.classList.remove('hidden');
            };
            reader.readAsDataURL(file);
        }
    }

    previewUrlImage(url: string) {
        const previewContainer = document.getElementById('url-image-preview-container');
        const previewImg = document.getElementById('url-image-preview') as HTMLImageElement;

        if (previewContainer && previewImg) {
            previewImg.onload = () => {
                previewContainer.classList.remove('hidden');
            };
            previewImg.onerror = () => {
                previewContainer.classList.add('hidden');
            };
            previewImg.src = url;
        }
    }

    async loadVaultImages(searchQuery: string = '') {
        if (!this.state.currentVaultId) return;

        const listContainer = document.getElementById('vault-images-list');
        if (!listContainer) return;

        listContainer.innerHTML = '<p>Loading images...</p>';

        try {
            const tree = await this.api.getFileTree(this.state.currentVaultId);
            const images = this.findImagesInTree(tree, searchQuery.toLowerCase());

            if (images.length === 0) {
                listContainer.innerHTML = '<p class="empty-state">No images found in vault</p>';
                return;
            }

            listContainer.innerHTML = '';
            images.forEach(imagePath => {
                const item = document.createElement('div');
                item.className = 'vault-image-item' + (this.selectedVaultImage === imagePath ? ' selected' : '');

                const fileName = imagePath.split('/').pop() || imagePath;
                const imgUrl = `/api/vaults/${this.state.currentVaultId}/raw/${imagePath}`;

                item.innerHTML = `
                    <img src="${imgUrl}" alt="${fileName}" loading="lazy">
                    <span class="image-name">${fileName}</span>
                `;

                item.addEventListener('click', () => {
                    // Deselect others
                    listContainer.querySelectorAll('.vault-image-item').forEach(i => i.classList.remove('selected'));
                    item.classList.add('selected');
                    this.selectedVaultImage = imagePath;
                });

                listContainer.appendChild(item);
            });
        } catch (error) {
            console.error('Failed to load vault images:', error);
            listContainer.innerHTML = '<p class="error">Failed to load images</p>';
        }
    }

    findImagesInTree(nodes: FileNode[], searchQuery: string = ''): string[] {
        const images: string[] = [];
        const imageExtensions = ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'];

        const traverse = (nodeList: FileNode[]) => {
            for (const node of nodeList) {
                if (node.is_directory && node.children) {
                    traverse(node.children);
                } else {
                    const ext = node.name.split('.').pop()?.toLowerCase();
                    if (ext && imageExtensions.includes(ext)) {
                        if (!searchQuery || node.name.toLowerCase().includes(searchQuery) || node.path.toLowerCase().includes(searchQuery)) {
                            images.push(node.path);
                        }
                    }
                }
            }
        };

        traverse(nodes);
        return images;
    }

    async handleInsertImage() {
        const activeTab = document.querySelector('.image-insert-tabs .tab-btn.active');
        const tabType = activeTab?.getAttribute('data-tab');
        const altText = (document.getElementById('image-alt-text') as HTMLInputElement)?.value.trim() || '';

        let markdown = '';

        if (tabType === 'upload' && this.selectedImageFile) {
            // Upload the image first
            if (!this.state.currentVaultId) {
                alert('Please select a vault first');
                return;
            }

            try {
                const fileList = this.createFileList([this.selectedImageFile]);
                const result = await this.api.uploadFiles(
                    this.state.currentVaultId,
                    fileList,
                    'attachments'
                );

                if (result.uploaded && result.uploaded.length > 0) {
                    const uploadedPath = result.uploaded[0].path;
                    markdown = altText ? `![[${uploadedPath}|${altText}]]` : `![[${uploadedPath}]]`;
                    this.loadFileTree(); // Refresh to show new file
                } else {
                    alert('Upload failed');
                    return;
                }
            } catch (error) {
                console.error('Failed to upload image:', error);
                alert('Failed to upload image: ' + error);
                return;
            }
        } else if (tabType === 'url') {
            const imageUrl = (document.getElementById('image-url') as HTMLInputElement)?.value.trim();
            if (!imageUrl) {
                alert('Please enter an image URL');
                return;
            }
            // Standard markdown image syntax for external URLs
            markdown = altText ? `![${altText}](${imageUrl})` : `![](${imageUrl})`;
        } else if (tabType === 'vault' && this.selectedVaultImage) {
            markdown = altText ? `![[${this.selectedVaultImage}|${altText}]]` : `![[${this.selectedVaultImage}]]`;
        } else {
            alert('Please select or upload an image');
            return;
        }

        this.insertTextAtCursor(markdown);
        this.hideModal('insert-image-modal');
    }

    createFileList(files: File[]): FileList {
        const dataTransfer = new DataTransfer();
        files.forEach(file => dataTransfer.items.add(file));
        return dataTransfer.files;
    }

    insertTextAtCursor(text: string) {
        const pane1 = document.getElementById('pane-1');
        if (!pane1) return;

        // Check textarea (raw or side-by-side mode)
        const textarea = pane1.querySelector('#editor-textarea') as HTMLTextAreaElement;
        if (textarea) {
            const start = textarea.selectionStart;
            const end = textarea.selectionEnd;
            const before = textarea.value.substring(0, start);
            const after = textarea.value.substring(end);

            textarea.value = before + text + after;
            textarea.selectionStart = textarea.selectionEnd = start + text.length;
            textarea.dispatchEvent(new Event('input', { bubbles: true }));
            textarea.focus();
            return;
        }

        // Check CodeJar (formatted mode)
        if (this.currentJar) {
            const editor = document.querySelector('#editor-formatted') as HTMLElement;
            if (editor) {
                const selection = window.getSelection();
                if (selection && selection.rangeCount > 0) {
                    const range = selection.getRangeAt(0);
                    range.deleteContents();
                    range.insertNode(document.createTextNode(text));
                    range.collapse(false);
                } else {
                    // Append at end
                    const currentContent = this.currentJar.toString();
                    this.currentJar.updateCode(currentContent + text);
                }
                return;
            }
        }

        // Check Quill (rendered mode)
        if (this.currentQuill) {
            const range = this.currentQuill.getSelection(true);
            const index = range ? range.index : this.currentQuill.getLength();

            // Check if it's an image
            const imageMatch = text.match(/!\[([^\]]*)\]\(([^)]+)\)/);
            if (imageMatch) {
                const imageUrl = imageMatch[2];
                this.currentQuill.insertEmbed(index, 'image', imageUrl, 'user');
            } else {
                this.currentQuill.insertText(index, text, 'user');
            }
            return;
        }
    }

    // Split Pane functionality
    // Split Pane functionality
    setupPanes() {
        const splitViewBtn = document.getElementById('split-view-btn');
        const toggleOrientationBtn = document.getElementById('toggle-orientation-btn');

        // Split view button - adds a new pane
        splitViewBtn?.addEventListener('click', () => {
            this.splitPane();
        });

        // Toggle orientation
        toggleOrientationBtn?.addEventListener('click', () => {
            this.state.splitOrientation = this.state.splitOrientation === 'vertical' ? 'horizontal' : 'vertical';
            this.renderEditor();
            this.savePreferences();
        });

        // Initial setup
        this.renderEditor();

        if (this.state.panes.length > 1) {
            toggleOrientationBtn?.classList.remove('hidden');
        } else {
            toggleOrientationBtn?.classList.add('hidden');
        }
    }

    splitPane() {
        const newPaneId = `pane-${Date.now()}`;
        this.state.panes.push({
            id: newPaneId,
            flex: 1,
            activeTabId: null
        });

        document.getElementById('toggle-orientation-btn')?.classList.remove('hidden');
        this.renderEditor();
        this.savePreferences();
    }

    closePane(paneId: string) {
        if (this.state.panes.length <= 1) return;

        const index = this.state.panes.findIndex(p => p.id === paneId);
        if (index === -1) return;

        const paneToRemove = this.state.panes[index];

        this.state.panes.splice(index, 1);

        if (this.state.activePaneId === paneId) {
            this.state.activePaneId = this.state.panes[0].id;
        }

        if (this.state.panes.length <= 1) {
            document.getElementById('toggle-orientation-btn')?.classList.add('hidden');
        }

        this.renderEditor();
        this.savePreferences();
    }

    setupResizer(resizer: HTMLElement, leftPane: any, rightPane: any) {
        let isDragging = false;
        let startPos = 0;
        let startLeftSize = 0;
        let startRightSize = 0;

        const leftEl = document.getElementById(leftPane.id);
        const rightEl = document.getElementById(rightPane.id);
        const splitContainer = document.getElementById('split-container');

        if (!leftEl || !rightEl || !splitContainer) return;

        resizer.addEventListener('mousedown', (e) => {
            isDragging = true;

            if (this.state.splitOrientation === 'vertical') {
                startPos = e.clientX;
                startLeftSize = leftEl.offsetWidth;
                startRightSize = rightEl.offsetWidth;
                document.body.style.cursor = 'col-resize';
            } else {
                startPos = e.clientY;
                startLeftSize = leftEl.offsetHeight;
                startRightSize = rightEl.offsetHeight;
                document.body.style.cursor = 'row-resize';
            }

            resizer.classList.add('dragging');
            document.body.style.userSelect = 'none';
            e.preventDefault();
        });

        document.addEventListener('mousemove', (e) => {
            if (!isDragging) return;

            const isVertical = this.state.splitOrientation === 'vertical';
            const currentPos = isVertical ? e.clientX : e.clientY;
            const delta = currentPos - startPos;

            const totalSize = startLeftSize + startRightSize;
            const newLeftSize = startLeftSize + delta;
            const newRightSize = startRightSize - delta;

            if (newLeftSize > 150 && newRightSize > 150) {
                const totalFlex = leftPane.flex + rightPane.flex;
                leftPane.flex = (newLeftSize / totalSize) * totalFlex;
                rightPane.flex = (newRightSize / totalSize) * totalFlex;

                leftEl.style.flex = leftPane.flex.toString();
                rightEl.style.flex = rightPane.flex.toString();
            }
        });

        document.addEventListener('mouseup', () => {
            if (isDragging) {
                isDragging = false;
                resizer.classList.remove('dragging');
                document.body.style.cursor = '';
                document.body.style.userSelect = '';
                this.savePreferences();
            }
        });
    }

    updatePaneActiveState() {
        this.state.panes.forEach(p => {
            const el = document.getElementById(p.id);
            if (el) {
                if (p.id === this.state.activePaneId) {
                    el.classList.add('active');
                } else {
                    el.classList.remove('active');
                }
            }
        });
    }

    openInSplitView(tabId: string) {
        this.splitPane();
        const newPane = this.state.panes[this.state.panes.length - 1];
        const tab = this.state.getTab(tabId);
        if (tab && newPane) {
            tab.pane = newPane.id;
            newPane.activeTabId = tabId;
            this.state.activePaneId = newPane.id;
            this.renderEditor();
            this.renderTabs();
            this.savePreferences();
        }
    }

    // In-File Search (Ctrl+F) functionality
    private inFileSearchMatches: { start: number; end: number }[] = [];
    private inFileSearchCurrentIndex: number = -1;
    private inFileSearchQuery: string = '';

    setupInFileSearch() {
        const searchBar = document.getElementById('in-file-search');
        const searchInput = document.getElementById('in-file-search-input') as HTMLInputElement;
        const searchCount = document.getElementById('in-file-search-count');
        const prevBtn = document.getElementById('in-file-search-prev');
        const nextBtn = document.getElementById('in-file-search-next');
        const closeBtn = document.getElementById('in-file-search-close');
        const caseCheckbox = document.getElementById('in-file-search-case') as HTMLInputElement;

        if (!searchBar || !searchInput || !searchCount || !prevBtn || !nextBtn || !closeBtn) {
            console.warn('In-file search elements not found');
            return;
        }

        // Ctrl+F to open search
        document.addEventListener('keydown', (e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
                // Only handle if we have an active tab with text content
                if (!this.state.activeTabId) return;
                const tab = this.state.getTab(this.state.activeTabId);
                if (!tab || (tab.fileType !== 'markdown' && tab.fileType !== 'text')) return;

                e.preventDefault();
                this.showInFileSearch();
            }
        });

        // Escape to close
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape' && !searchBar.classList.contains('hidden')) {
                this.hideInFileSearch();
            }
        });

        // Close button
        closeBtn.addEventListener('click', () => {
            this.hideInFileSearch();
        });

        // Search input handler
        searchInput.addEventListener('input', () => {
            this.performInFileSearch();
        });

        // Case sensitivity toggle
        caseCheckbox.addEventListener('change', () => {
            this.performInFileSearch();
        });

        // Enter to go to next, Shift+Enter to go to previous
        searchInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                e.preventDefault();
                if (e.shiftKey) {
                    this.goToPreviousMatch();
                } else {
                    this.goToNextMatch();
                }
            }
        });

        // Navigation buttons
        prevBtn.addEventListener('click', () => this.goToPreviousMatch());
        nextBtn.addEventListener('click', () => this.goToNextMatch());
    }

    showInFileSearch() {
        const searchBar = document.getElementById('in-file-search');
        const searchInput = document.getElementById('in-file-search-input') as HTMLInputElement;

        if (searchBar && searchInput) {
            searchBar.classList.remove('hidden');
            searchInput.focus();
            searchInput.select();

            // If there's already a search query, perform search
            if (searchInput.value) {
                this.performInFileSearch();
            }
        }
    }

    hideInFileSearch() {
        const searchBar = document.getElementById('in-file-search');
        if (searchBar) {
            searchBar.classList.add('hidden');
        }
        this.clearSearchHighlights();
        this.inFileSearchMatches = [];
        this.inFileSearchCurrentIndex = -1;
    }

    performInFileSearch() {
        const searchInput = document.getElementById('in-file-search-input') as HTMLInputElement;
        const caseCheckbox = document.getElementById('in-file-search-case') as HTMLInputElement;
        const searchCount = document.getElementById('in-file-search-count');

        if (!searchInput || !searchCount) return;

        const query = searchInput.value;
        const caseSensitive = caseCheckbox?.checked || false;

        this.inFileSearchQuery = query;
        this.inFileSearchMatches = [];
        this.inFileSearchCurrentIndex = -1;

        // Clear previous highlights
        this.clearSearchHighlights();

        if (!query) {
            searchCount.textContent = '';
            searchCount.classList.remove('no-results');
            return;
        }

        // Get current content
        const tab = this.state.activeTabId ? this.state.getTab(this.state.activeTabId) : null;
        if (!tab) return;

        const content = tab.content;
        const searchContent = caseSensitive ? content : content.toLowerCase();
        const searchQuery = caseSensitive ? query : query.toLowerCase();

        // Find all matches
        let pos = 0;
        while (pos < searchContent.length) {
            const index = searchContent.indexOf(searchQuery, pos);
            if (index === -1) break;
            this.inFileSearchMatches.push({ start: index, end: index + query.length });
            pos = index + 1;
        }

        // Update count display
        if (this.inFileSearchMatches.length === 0) {
            searchCount.textContent = 'No results';
            searchCount.classList.add('no-results');
        } else {
            this.inFileSearchCurrentIndex = 0;
            searchCount.textContent = `1 of ${this.inFileSearchMatches.length}`;
            searchCount.classList.remove('no-results');
            this.highlightMatches();
            this.scrollToCurrentMatch();
        }
    }

    goToNextMatch() {
        if (this.inFileSearchMatches.length === 0) return;

        this.inFileSearchCurrentIndex = (this.inFileSearchCurrentIndex + 1) % this.inFileSearchMatches.length;
        this.updateSearchCountDisplay();
        this.highlightMatches();
        this.scrollToCurrentMatch();
    }

    goToPreviousMatch() {
        if (this.inFileSearchMatches.length === 0) return;

        this.inFileSearchCurrentIndex = this.inFileSearchCurrentIndex <= 0
            ? this.inFileSearchMatches.length - 1
            : this.inFileSearchCurrentIndex - 1;
        this.updateSearchCountDisplay();
        this.highlightMatches();
        this.scrollToCurrentMatch();
    }

    updateSearchCountDisplay() {
        const searchCount = document.getElementById('in-file-search-count');
        if (searchCount && this.inFileSearchMatches.length > 0) {
            searchCount.textContent = `${this.inFileSearchCurrentIndex + 1} of ${this.inFileSearchMatches.length}`;
        }
    }

    highlightMatches() {
        // Different highlighting strategies based on editor mode
        const pane1 = document.getElementById('pane-1');
        if (!pane1) return;

        // For textarea (raw or side-by-side mode)
        const textarea = pane1.querySelector('#editor-textarea') as HTMLTextAreaElement;
        if (textarea && this.inFileSearchMatches.length > 0) {
            // Textareas can't have HTML highlights, so we just select the current match
            const currentMatch = this.inFileSearchMatches[this.inFileSearchCurrentIndex];
            if (currentMatch) {
                textarea.setSelectionRange(currentMatch.start, currentMatch.end);
            }
            return;
        }

        // For CodeJar (formatted mode)
        const formattedEditor = pane1.querySelector('#editor-formatted') as HTMLElement;
        if (formattedEditor && this.currentJar) {
            this.highlightInContentEditable(formattedEditor);
            return;
        }

        // For Quill (rendered mode)
        if (this.currentQuill) {
            this.highlightInQuill();
            return;
        }

        // For code viewer (read-only text files)
        const codeViewer = pane1.querySelector('.code-viewer code') as HTMLElement;
        if (codeViewer) {
            this.highlightInCodeViewer(codeViewer);
            return;
        }
    }

    highlightInContentEditable(editor: HTMLElement) {
        if (!this.inFileSearchQuery || this.inFileSearchMatches.length === 0) return;

        // Get the text content and current code
        const content = this.currentJar ? this.currentJar.toString() : editor.textContent || '';

        // Escape HTML and add highlights
        let highlightedContent = this.escapeHtml(content);

        // Sort matches in reverse order to replace from end to start
        const sortedMatches = [...this.inFileSearchMatches].sort((a, b) => b.start - a.start);

        for (let i = sortedMatches.length - 1; i >= 0; i--) {
            const match = sortedMatches[i];
            const originalIndex = this.inFileSearchMatches.indexOf(match);
            const isCurrent = originalIndex === this.inFileSearchCurrentIndex;
            const className = isCurrent ? 'search-highlight current' : 'search-highlight';

            const before = highlightedContent.substring(0, match.start);
            const matchText = highlightedContent.substring(match.start, match.end);
            const after = highlightedContent.substring(match.end);

            highlightedContent = before + `<mark class="${className}">${matchText}</mark>` + after;
        }

        // Temporarily disable the CodeJar update listener
        editor.innerHTML = highlightedContent;

        // Re-apply syntax highlighting
        hljs.highlightElement(editor);
    }

    highlightInQuill() {
        if (!this.currentQuill || this.inFileSearchMatches.length === 0) return;

        // Quill uses a different approach - we format the text
        // First remove any existing search highlights
        const length = this.currentQuill.getLength();
        this.currentQuill.formatText(0, length, 'background', false, 'silent');

        // Apply highlights
        for (let i = 0; i < this.inFileSearchMatches.length; i++) {
            const match = this.inFileSearchMatches[i];
            const isCurrent = i === this.inFileSearchCurrentIndex;
            const color = isCurrent ? 'rgba(255, 165, 0, 0.7)' : 'rgba(255, 215, 0, 0.4)';
            this.currentQuill.formatText(match.start, match.end - match.start, 'background', color, 'silent');
        }
    }

    highlightInCodeViewer(codeElement: HTMLElement) {
        if (!this.inFileSearchQuery || this.inFileSearchMatches.length === 0) return;

        const tab = this.state.activeTabId ? this.state.getTab(this.state.activeTabId) : null;
        if (!tab) return;

        // Escape HTML and add highlights
        let highlightedContent = this.escapeHtml(tab.content);

        // Sort matches in reverse order
        const sortedMatches = [...this.inFileSearchMatches].sort((a, b) => b.start - a.start);

        for (const match of sortedMatches) {
            const originalIndex = this.inFileSearchMatches.indexOf(match);
            const isCurrent = originalIndex === this.inFileSearchCurrentIndex;
            const className = isCurrent ? 'search-highlight current' : 'search-highlight';

            const before = highlightedContent.substring(0, match.start);
            const matchText = highlightedContent.substring(match.start, match.end);
            const after = highlightedContent.substring(match.end);

            highlightedContent = before + `<mark class="${className}">${matchText}</mark>` + after;
        }

        codeElement.innerHTML = highlightedContent;
        hljs.highlightElement(codeElement);
    }

    scrollToCurrentMatch() {
        if (this.inFileSearchCurrentIndex < 0 || this.inFileSearchMatches.length === 0) return;

        const currentMatch = this.inFileSearchMatches[this.inFileSearchCurrentIndex];
        const pane1 = document.getElementById('pane-1');
        if (!pane1) return;

        // For textarea
        const textarea = pane1.querySelector('#editor-textarea') as HTMLTextAreaElement;
        if (textarea) {
            textarea.focus();
            // Calculate approximate scroll position based on line
            const content = textarea.value.substring(0, currentMatch.start);
            const lines = content.split('\n').length;
            const lineHeight = parseInt(getComputedStyle(textarea).lineHeight) || 20;
            textarea.scrollTop = Math.max(0, (lines - 5) * lineHeight);
            return;
        }

        // For formatted editor or code viewer - scroll to highlighted element
        const currentHighlight = pane1.querySelector('.search-highlight.current');
        if (currentHighlight) {
            currentHighlight.scrollIntoView({ behavior: 'smooth', block: 'center' });
            return;
        }

        // For Quill
        if (this.currentQuill) {
            this.currentQuill.setSelection(currentMatch.start, currentMatch.end - currentMatch.start);
            return;
        }
    }

    clearSearchHighlights() {
        const pane1 = document.getElementById('pane-1');
        if (!pane1) return;

        // For CodeJar - restore original content
        if (this.currentJar) {
            const editor = pane1.querySelector('#editor-formatted') as HTMLElement;
            if (editor) {
                const content = this.currentJar.toString();
                this.currentJar.updateCode(content, false);
            }
        }

        // For Quill - remove background formatting
        if (this.currentQuill) {
            const length = this.currentQuill.getLength();
            this.currentQuill.formatText(0, length, 'background', false, 'silent');
        }

        // For code viewer - re-render without highlights
        const codeViewer = pane1.querySelector('.code-viewer code') as HTMLElement;
        if (codeViewer) {
            const tab = this.state.activeTabId ? this.state.getTab(this.state.activeTabId) : null;
            if (tab) {
                codeViewer.textContent = tab.content;
                hljs.highlightElement(codeViewer);
            }
        }
    }
    async loadPreferences() {
        try {
            const prefs = await this.api.getPreferences();

            // Set theme
            if (prefs.theme) {
                document.body.className = `theme-${prefs.theme}`;
                // Update toggle button icon/state if needed
                const themeBtn = document.getElementById('theme-toggle-btn');
                if (themeBtn) {
                    const icon = themeBtn.querySelector('.icon');
                    if (icon) icon.textContent = prefs.theme === 'light' ? '☀️' : '🌙';
                }
            }

            // Set editor mode
            if (prefs.editor_mode) {
                const mapBackendToFrontend: any = {
                    'raw': 'raw',
                    'side_by_side': 'side-by-side',
                    'formatted_raw': 'formatted',
                    'fully_rendered': 'rendered'
                };
                const mode = mapBackendToFrontend[prefs.editor_mode];
                if (mode) {
                    this.state.editorMode = mode;
                }
            }

            // Set window layout
            if (prefs.window_layout) {
                try {
                    const layout = JSON.parse(prefs.window_layout);
                    if (layout.panes && Array.isArray(layout.panes)) {
                        // Validate panes
                        this.state.panes = layout.panes;
                        this.state.splitOrientation = layout.splitOrientation || 'vertical';
                        this.state.activePaneId = layout.activePaneId || (this.state.panes[0]?.id || 'pane-1');

                        // We need to clear tabs that might be referenced in panes but not open? 
                        // Actually tabs are not persisted in layout yet, just the panes structure.
                        // Reset activeTabIds if they refer to non-existent tabs (tabs are closed on reload)
                        this.state.panes.forEach(p => p.activeTabId = null);
                    }
                } catch (e) {
                    console.error('Failed to parse window layout', e);
                }
            }

        } catch (e) {
            console.error('Failed to load preferences', e);
        }
    }

    async savePreferences() {
        const mapFrontendToBackend: any = {
            'raw': 'raw',
            'side-by-side': 'side_by_side',
            'formatted': 'formatted_raw',
            'rendered': 'fully_rendered'
        };

        const currentTheme = document.body.classList.contains('theme-light') ? 'light' : 'dark';

        const layout = {
            panes: this.state.panes,
            splitOrientation: this.state.splitOrientation,
            activePaneId: this.state.activePaneId
        };

        const prefs: UserPreferences = {
            theme: currentTheme,
            editor_mode: mapFrontendToBackend[this.state.editorMode] || 'side_by_side',
            font_size: 14, // Default for now
            window_layout: JSON.stringify(layout)
        };

        try {
            await this.api.updatePreferences(prefs);
        } catch (e) {
            console.error('Failed to save preferences', e);
        }
    }

    async loadRecentFiles(vaultId: string) {
        try {
            const files = await this.api.getRecentFiles(vaultId);
            this.state.recentFiles = files;
        } catch (e) {
            console.error('Failed to load recent files', e);
        }
    }

    setupPreferencesEvents() {
        const themeBtn = document.getElementById('theme-toggle-btn');
        if (themeBtn) {
            // Clone to remove existing listeners (if any) to prevent double toggling
            const newThemeBtn = themeBtn.cloneNode(true);
            themeBtn.parentNode?.replaceChild(newThemeBtn, themeBtn);

            newThemeBtn.addEventListener('click', () => {
                const isLight = document.body.classList.contains('theme-light');
                document.body.classList.toggle('theme-light', !isLight);
                document.body.classList.toggle('theme-dark', isLight);

                const icon = (newThemeBtn as HTMLElement).querySelector('.icon');
                if (icon) icon.textContent = !isLight ? '☀️' : '🌙';

                this.savePreferences();
            });
        }

        // Mode buttons are dynamic per pane, so we need delegated listeners or hook into renderPaneContent
        // Looking at renderPaneContent, it creates buttons.
        // We probably need to hook into `setEditorMode` if it exists.
    }

    setupFileTreeEvents() {
        const container = document.getElementById('file-tree');
        if (!container) return;

        // Delegated click handler
        container.addEventListener('click', (e) => {
            const target = e.target as HTMLElement;

            // Handle folder arrow click for expand/collapse
            if (target.classList.contains('folder-arrow')) {
                e.stopPropagation();
                const nodeWrapper = target.closest('.file-tree-node');
                if (nodeWrapper) {
                    nodeWrapper.classList.toggle('collapsed');
                    if (nodeWrapper.classList.contains('collapsed')) {
                        target.textContent = '▶';
                    } else {
                        target.textContent = '▼';
                    }
                }
                return;
            }

            const item = target.closest('.file-tree-item');
            if (!item) return;

            // Handle file click
            if (!item.classList.contains('folder')) {
                const path = item.getAttribute('data-path');
                if (path) this.openFile(path);
            }
        });

        // Delegated context menu handler
        container.addEventListener('contextmenu', (e) => {
            e.preventDefault();
            const target = e.target as HTMLElement;
            const item = target.closest('.file-tree-item');
            if (!item) return;

            const path = item.getAttribute('data-path');
            if (!path) return;

            const isDirectory = item.classList.contains('folder');
            const name = item.querySelector('.file-name')?.textContent || item.querySelector('.name')?.textContent || '';

            // Construct a fake FileNode for existing helper
            const node: FileNode = {
                name: name,
                path: path,
                is_directory: isDirectory
            };
            this.showFileContextMenu(e, node);
        });

        // Setup drop zone for file uploads from OS
        container.addEventListener('dragover', (e) => {
            e.preventDefault();
            e.stopPropagation();
            if (e.dataTransfer) {
                // Only allow if dragging files from OS (not internal files)
                if (e.dataTransfer.types.includes('Files') && !e.dataTransfer.types.includes('application/obsidian-file')) {
                    e.dataTransfer.dropEffect = 'copy';
                    container.classList.add('file-tree-drag-over');
                }
            }
        });

        container.addEventListener('dragleave', (e) => {
            e.preventDefault();
            e.stopPropagation();
            // Only remove class if leaving the container entirely
            const relatedTarget = e.relatedTarget as HTMLElement;
            if (!container.contains(relatedTarget)) {
                container.classList.remove('file-tree-drag-over');
            }
        });

        container.addEventListener('drop', async (e) => {
            e.preventDefault();
            e.stopPropagation();
            container.classList.remove('file-tree-drag-over');

            // Only handle files from OS, not internal drag operations
            if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
                // Upload to root of vault
                await this.handleFileUploadDrop(e.dataTransfer.files, '');
            }
        });
    }
}

// Initialize the app
document.addEventListener('DOMContentLoaded', async () => {
    const state = new AppState();
    const api = new ApiClient();
    const ui = new UIManager(state, api);

    // Expose UI manager to window for testing
    (window as any).ui = ui;
    (window as any).app = ui; // Also expose as app for compatibility

    await ui.loadPreferences();
    await ui.loadVaults();
    ui.setupEventListeners();
    ui.setupPreferencesEvents();
    ui.setupWebSocket();
    ui.setupQuickSwitcher();
    ui.setupTemplates();
    ui.setupConflictResolution();
    ui.setupUndoRedo();
    ui.setupInsertHelpers();
    ui.setupInFileSearch();
    ui.setupPanes();
    ui.setupFileTreeEvents();
    ui.setupPluginManager();
});
