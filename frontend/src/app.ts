// Types
interface Vault {
    id: string;
    name: string;
    path: string;
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
    pane: number;
    fileType: 'markdown' | 'image' | 'pdf' | 'text' | 'other';
    frontmatter?: any;
}

// File type detection helpers
function getFileType(filePath: string): 'markdown' | 'image' | 'pdf' | 'text' | 'other' {
    const ext = filePath.split('.').pop()?.toLowerCase();
    if (!ext) return 'other';

    if (ext === 'md') return 'markdown';
    if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) return 'image';
    if (ext === 'pdf') return 'pdf';
    if (['txt', 'json', 'js', 'ts', 'css', 'html', 'xml'].includes(ext)) return 'text';
    return 'other';
}

function isImageFile(filePath: string): boolean {
    return getFileType(filePath) === 'image';
}

// App State
class AppState {
    currentVaultId: string | null = null;
    vaults: Vault[] = [];
    openTabs: Map<string, Tab> = new Map();
    activeTabId: string | null = null;
    editorMode: 'raw' | 'side-by-side' | 'formatted' | 'rendered' = 'raw';
    ws: WebSocket | null = null;
    searchDebounce: number | null = null;

    setVault(vaultId: string) {
        this.currentVaultId = vaultId;
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
class ApiClient {
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

    async search(vaultId: string, query: string, limit: number = 50): Promise<SearchResult[]> {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/search?q=${encodeURIComponent(query)}&limit=${limit}`);
        if (!response.ok) throw new Error('Failed to search');
        return response.json();
    }

    async uploadFiles(vaultId: string, files: FileList, targetPath: string = '', onProgress?: (loaded: number, total: number) => void): Promise<any> {
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
}

// UI Manager
class UIManager {
    constructor(private state: AppState, private api: ApiClient) {}

    async loadVaults() {
        try {
            this.state.vaults = await this.api.getVaults();
            this.renderVaultSelector();
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
            option.textContent = vault.name;
            if (vault.id === this.state.currentVaultId) {
                option.selected = true;
            }
            select.appendChild(option);
        }
    }

    async switchVault(vaultId: string) {
        if (!vaultId) return;

        this.state.setVault(vaultId);
        this.closeAllTabs();
        await this.loadFileTree();
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
        }

        for (const node of nodes) {
            const item = document.createElement('div');
            item.className = 'file-tree-item' + (node.is_directory ? ' folder' : '');
            item.innerHTML = `
                <span class="icon">${node.is_directory ? '📁' : '📄'}</span>
                <span class="name">${node.name}</span>
            `;

            if (!node.is_directory) {
                item.addEventListener('click', () => this.openFile(node.path));
            }

            // Add context menu support
            item.addEventListener('contextmenu', (e) => {
                e.preventDefault();
                this.showFileContextMenu(e, node);
            });

            container.appendChild(item);

            if (node.is_directory && node.children && node.children.length > 0) {
                const childContainer = document.createElement('div');
                childContainer.className = 'file-tree-children';
                container.appendChild(childContainer);
                this.renderFileTree(node.children, childContainer);
            }
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

        const downloadOption = document.createElement('div');
        downloadOption.className = 'context-menu-item';
        downloadOption.textContent = node.is_directory ? 'Download as ZIP' : 'Download';
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

    async openFile(filePath: string) {
        if (!this.state.currentVaultId) return;

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
            } else if (fileType === 'image' || fileType === 'pdf') {
                // For binary files, we'll use the raw endpoint directly
                content = `/api/vaults/${this.state.currentVaultId}/raw/${filePath}`;
            }

            const tab: Tab = {
                id: tabId,
                filePath: filePath,
                fileName: filePath.split('/').pop() || filePath,
                content: content,
                modified: modified,
                isDirty: false,
                pane: 1,
                fileType: fileType,
                frontmatter: frontmatter,
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
        } catch (error) {
            console.error('Failed to save file:', error);
            alert('Failed to save file: ' + error);
        }
    }

    renderTabs() {
        const tabsContainer = document.getElementById('tabs');
        if (!tabsContainer) return;

        tabsContainer.innerHTML = '';

        for (const [tabId, tab] of this.state.openTabs) {
            const tabElement = document.createElement('div');
            tabElement.className = 'tab' + (tabId === this.state.activeTabId ? ' active' : '');
            tabElement.innerHTML = `
                <span class="tab-name">${tab.isDirty ? '• ' : ''}${tab.fileName}</span>
                <button class="tab-close">✕</button>
            `;

            tabElement.querySelector('.tab-name')?.addEventListener('click', () => this.activateTab(tabId));
            tabElement.querySelector('.tab-close')?.addEventListener('click', (e) => {
                e.stopPropagation();
                this.closeTab(tabId);
            });

            tabsContainer.appendChild(tabElement);
        }
    }

    activateTab(tabId: string) {
        this.state.setActiveTab(tabId);
        this.renderTabs();
        this.renderEditor();

        // Update properties panel if it's open
        const propertiesPanel = document.getElementById('properties-panel');
        if (propertiesPanel && !propertiesPanel.classList.contains('hidden')) {
            this.renderProperties();
        }
    }

    closeTab(tabId: string) {
        const tab = this.state.getTab(tabId);
        if (tab?.isDirty) {
            if (!confirm('File has unsaved changes. Close anyway?')) {
                return;
            }
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
        this.state.openTabs.clear();
        this.state.setActiveTab(null);
        this.renderTabs();
        this.renderEditor();
    }

    renderEditor() {
        const pane1 = document.getElementById('pane-1');
        if (!pane1) return;

        const content = pane1.querySelector('.editor-content') as HTMLElement;
        if (!content) return;

        if (!this.state.activeTabId) {
            content.innerHTML = `
                <div class="empty-state">
                    <h2>No file open</h2>
                    <p>Select a file from the sidebar to start editing</p>
                </div>
            `;
            return;
        }

        const tab = this.state.getTab(this.state.activeTabId);
        if (!tab) return;

        // Handle different file types
        if (tab.fileType === 'image') {
            this.renderImageViewer(content, tab);
            return;
        } else if (tab.fileType === 'pdf') {
            this.renderPdfViewer(content, tab);
            return;
        } else if (tab.fileType === 'other') {
            this.renderUnsupportedFile(content, tab);
            return;
        }

        // For markdown and text files, use the editor modes
        switch (this.state.editorMode) {
            case 'raw':
                this.renderRawEditor(content, tab);
                break;
            case 'side-by-side':
                this.renderSideBySideEditor(content, tab);
                break;
            case 'formatted':
                this.renderFormattedEditor(content, tab);
                break;
            case 'rendered':
                this.renderRenderedEditor(content, tab);
                break;
        }
    }

    renderRawEditor(container: HTMLElement, tab: Tab) {
        container.innerHTML = `<textarea class="editor-raw" id="editor-textarea">${tab.content}</textarea>`;

        const textarea = container.querySelector('#editor-textarea') as HTMLTextAreaElement;
        if (textarea) {
            textarea.addEventListener('input', () => {
                tab.content = textarea.value;
                tab.isDirty = true;
                this.renderTabs();
            });

            // Auto-save every 5 seconds
            setInterval(() => {
                if (tab.isDirty) {
                    this.saveFile(tab.id);
                }
            }, 5000);
        }
    }

    renderSideBySideEditor(container: HTMLElement, tab: Tab) {
        container.innerHTML = `
            <div class="editor-side-by-side">
                <div>
                    <textarea class="editor-raw" id="editor-textarea">${tab.content}</textarea>
                </div>
                <div class="editor-preview markdown-content" id="preview-pane"></div>
            </div>
        `;

        const textarea = container.querySelector('#editor-textarea') as HTMLTextAreaElement;
        const preview = container.querySelector('#preview-pane') as HTMLElement;

        if (textarea && preview) {
            const updatePreview = () => {
                preview.innerHTML = this.renderMarkdown(textarea.value);
            };

            textarea.addEventListener('input', () => {
                tab.content = textarea.value;
                tab.isDirty = true;
                updatePreview();
                this.renderTabs();
            });

            updatePreview();
        }
    }

    renderFormattedEditor(container: HTMLElement, tab: Tab) {
        // Similar to raw but with syntax highlighting (simplified for now)
        this.renderRawEditor(container, tab);
    }

    renderRenderedEditor(container: HTMLElement, tab: Tab) {
        container.innerHTML = `<div class="markdown-content">${this.renderMarkdown(tab.content)}</div>`;
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
            <div class="pdf-viewer">
                <iframe src="${tab.content}" width="100%" height="100%" style="border: none;"></iframe>
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

    renderMarkdown(content: string): string {
        // Basic markdown rendering (simplified)
        // In production, use a library like marked.js
        let html = content
            .replace(/^### (.*$)/gim, '<h3>$1</h3>')
            .replace(/^## (.*$)/gim, '<h2>$1</h2>')
            .replace(/^# (.*$)/gim, '<h1>$1</h1>')
            .replace(/\*\*(.*)\*\*/gim, '<strong>$1</strong>')
            .replace(/\*(.*)\*/gim, '<em>$1</em>')
            .replace(/\n/gim, '<br>');

        return html;
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
        addVaultForm?.addEventListener('submit', async (e) => {
            e.preventDefault();
            const formData = new FormData(addVaultForm);
            const name = formData.get('name') as string;
            const path = formData.get('path') as string;

            try {
                await this.api.createVault(name, path);
                await this.loadVaults();
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

        // Upload functionality
        this.setupUploadHandlers();
        this.setupDragAndDrop();
    }

    setupUploadHandlers() {
        const uploadBtn = document.getElementById('upload-btn');
        const browseBtn = document.getElementById('browse-btn');
        const fileInput = document.getElementById('file-input') as HTMLInputElement;
        const uploadArea = document.getElementById('upload-area');

        uploadBtn?.addEventListener('click', () => {
            this.showModal('upload-modal');
        });

        browseBtn?.addEventListener('click', () => {
            fileInput?.click();
        });

        uploadArea?.addEventListener('click', (e) => {
            if (e.target === uploadArea || (e.target as HTMLElement).closest('.upload-prompt')) {
                fileInput?.click();
            }
        });

        fileInput?.addEventListener('change', (e) => {
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

        ws.onmessage = (event) => {
            try {
                const changeEvent = JSON.parse(event.data);
                console.log('File change event:', changeEvent);

                // Reload file tree if current vault
                if (changeEvent.vault_id === this.state.currentVaultId) {
                    this.loadFileTree();
                }
            } catch (error) {
                console.error('Failed to parse WebSocket message:', error);
            }
        };

        ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };

        ws.onclose = () => {
            console.log('WebSocket closed, reconnecting in 5s...');
            setTimeout(() => this.setupWebSocket(), 5000);
        };

        this.state.ws = ws;
    }
}

// Initialize the app
document.addEventListener('DOMContentLoaded', async () => {
    const state = new AppState();
    const api = new ApiClient();
    const ui = new UIManager(state, api);

    await ui.loadVaults();
    ui.setupEventListeners();
    ui.setupWebSocket();
});
