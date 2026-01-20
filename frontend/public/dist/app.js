import hljs from 'highlight.js';
// @ts-ignore
import { CodeJar } from './vendor/codejar/codejar.js';
// File type detection helpers
function getFileType(filePath) {
    const ext = filePath.split('.').pop()?.toLowerCase();
    if (!ext)
        return 'other';
    if (ext === 'md')
        return 'markdown';
    if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext))
        return 'image';
    if (ext === 'pdf')
        return 'pdf';
    if (['mp3', 'wav', 'ogg'].includes(ext))
        return 'audio';
    if (['mp4', 'webm'].includes(ext))
        return 'video';
    if (['txt', 'json', 'js', 'ts', 'css', 'html', 'xml'].includes(ext))
        return 'text';
    return 'other';
}
function isImageFile(filePath) {
    return getFileType(filePath) === 'image';
}
// Utility
function debounce(func, wait) {
    let timeout;
    return function (...args) {
        clearTimeout(timeout);
        timeout = setTimeout(() => func.apply(this, args), wait);
    };
}
// App State
class AppState {
    constructor() {
        this.currentVaultId = null;
        this.vaults = [];
        this.openTabs = new Map();
        this.activeTabId = null;
        this._editorMode = 'raw';
        this.ws = null;
        this.searchDebounce = null;
        this.quickSwitcherDebounce = null;
        this.recentFiles = [];
        this.wsReconnectAttempts = 0;
        this.wsReconnectTimeout = null;
        this.wsMaxReconnectDelay = 30000; // 30 seconds max
        this.conflictData = null;
        const saved = localStorage.getItem('editor-mode');
        if (saved && ['raw', 'side-by-side', 'formatted', 'rendered'].includes(saved)) {
            this._editorMode = saved;
        }
    }
    get editorMode() {
        return this._editorMode;
    }
    set editorMode(mode) {
        this._editorMode = mode;
        localStorage.setItem('editor-mode', mode);
    }
    setVault(vaultId) {
        this.currentVaultId = vaultId;
        this.loadRecentFiles();
    }
    addRecentFile(filePath) {
        if (!this.currentVaultId)
            return;
        // Remove if exists to move to top
        this.recentFiles = this.recentFiles.filter(p => p !== filePath);
        // Add to front
        this.recentFiles.unshift(filePath);
        // Limit to 20
        if (this.recentFiles.length > 20) {
            this.recentFiles.pop();
        }
        this.saveRecentFiles();
    }
    saveRecentFiles() {
        if (!this.currentVaultId)
            return;
        localStorage.setItem(`recent-files-${this.currentVaultId}`, JSON.stringify(this.recentFiles));
    }
    loadRecentFiles() {
        if (!this.currentVaultId) {
            this.recentFiles = [];
            return;
        }
        const stored = localStorage.getItem(`recent-files-${this.currentVaultId}`);
        if (stored) {
            try {
                this.recentFiles = JSON.parse(stored);
            }
            catch (e) {
                console.error('Failed to parse recent files', e);
                this.recentFiles = [];
            }
        }
        else {
            this.recentFiles = [];
        }
    }
    addTab(tab) {
        this.openTabs.set(tab.id, tab);
    }
    removeTab(tabId) {
        this.openTabs.delete(tabId);
    }
    getTab(tabId) {
        return this.openTabs.get(tabId);
    }
    setActiveTab(tabId) {
        this.activeTabId = tabId;
    }
}
// API Client
class ApiClient {
    constructor() {
        this.baseUrl = '';
    }
    async getVaults() {
        const response = await fetch(`${this.baseUrl}/api/vaults`);
        if (!response.ok)
            throw new Error('Failed to fetch vaults');
        return response.json();
    }
    async createVault(name, path) {
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
    async deleteVault(vaultId) {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}`, {
            method: 'DELETE',
        });
        if (!response.ok)
            throw new Error('Failed to delete vault');
    }
    async getFileTree(vaultId) {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/files`);
        if (!response.ok)
            throw new Error('Failed to fetch file tree');
        return response.json();
    }
    async readFile(vaultId, filePath) {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/files/${filePath}`);
        if (!response.ok)
            throw new Error('Failed to read file');
        return response.json();
    }
    async writeFile(vaultId, filePath, content, lastModified, frontmatter) {
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
    async createFile(vaultId, filePath, content) {
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
    async deleteFile(vaultId, filePath) {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/files/${filePath}`, {
            method: 'DELETE',
        });
        if (!response.ok)
            throw new Error('Failed to delete file');
    }
    async createDirectory(vaultId, path) {
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
    async search(vaultId, query, limit = 50) {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/search?q=${encodeURIComponent(query)}&limit=${limit}`);
        if (!response.ok)
            throw new Error('Failed to search');
        return response.json();
    }
    async getRandomNote(vaultId) {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/random`);
        if (!response.ok)
            throw new Error('Failed to get random note');
        return response.json();
    }
    async getDailyNote(vaultId, date) {
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
    async uploadFiles(vaultId, files, targetPath = '', onProgress) {
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
                }
                else {
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
    async downloadFile(vaultId, filePath) {
        const url = `${this.baseUrl}/api/vaults/${vaultId}/download/${filePath}`;
        const link = document.createElement('a');
        link.href = url;
        link.download = filePath.split('/').pop() || 'download';
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
    }
    async downloadZip(vaultId, paths) {
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
    async renderMarkdown(content) {
        const response = await fetch(`${this.baseUrl}/api/render`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ content }),
        });
        if (!response.ok)
            throw new Error('Failed to render markdown');
        return response.text();
    }
}
// UI Manager
class UIManager {
    constructor(state, api) {
        this.state = state;
        this.api = api;
        this.currentJar = null;
        this.currentQuill = null;
    }
    async loadVaults() {
        try {
            this.state.vaults = await this.api.getVaults();
            this.renderVaultSelector();
        }
        catch (error) {
            console.error('Failed to load vaults:', error);
            alert('Failed to load vaults: ' + error);
        }
    }
    renderVaultSelector() {
        const select = document.getElementById('vault-select');
        if (!select)
            return;
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
    async switchVault(vaultId) {
        if (!vaultId)
            return;
        this.state.setVault(vaultId);
        this.closeAllTabs();
        await this.loadFileTree();
    }
    async loadFileTree() {
        if (!this.state.currentVaultId)
            return;
        try {
            const tree = await this.api.getFileTree(this.state.currentVaultId);
            this.renderFileTree(tree);
        }
        catch (error) {
            console.error('Failed to load file tree:', error);
            alert('Failed to load file tree: ' + error);
        }
    }
    renderFileTree(nodes, parentElement) {
        const container = parentElement || document.getElementById('file-tree');
        if (!container)
            return;
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
    showFileContextMenu(event, node) {
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
            if (!this.state.currentVaultId)
                return;
            try {
                if (node.is_directory) {
                    await this.api.downloadZip(this.state.currentVaultId, [node.path]);
                }
                else {
                    await this.api.downloadFile(this.state.currentVaultId, node.path);
                }
            }
            catch (error) {
                console.error('Download failed:', error);
                alert('Failed to download: ' + error);
            }
            menu.remove();
        });
        menu.appendChild(downloadOption);
        document.body.appendChild(menu);
        // Close menu when clicking outside
        const closeMenu = (e) => {
            if (!menu.contains(e.target)) {
                menu.remove();
                document.removeEventListener('click', closeMenu);
            }
        };
        setTimeout(() => {
            document.addEventListener('click', closeMenu);
        }, 0);
    }
    async openFile(filePath) {
        if (!this.state.currentVaultId)
            return;
        // Add to recents
        this.state.addRecentFile(filePath);
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
            }
            else if (['image', 'pdf', 'audio', 'video'].includes(fileType)) {
                // For binary files, we'll use the raw endpoint directly
                content = `/api/vaults/${this.state.currentVaultId}/raw/${filePath}`;
            }
            const tab = {
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
        }
        catch (error) {
            console.error('Failed to open file:', error);
            alert('Failed to open file: ' + error);
        }
    }
    async saveFile(tabId) {
        const tab = this.state.getTab(tabId);
        if (!tab || !this.state.currentVaultId)
            return;
        const saveStatus = document.getElementById('save-status');
        if (saveStatus) {
            saveStatus.textContent = 'Saving...';
            saveStatus.className = 'save-status saving';
        }
        try {
            const updated = await this.api.writeFile(this.state.currentVaultId, tab.filePath, tab.content, tab.modified, tab.frontmatter);
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
        }
        catch (error) {
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
        if (!tabsContainer)
            return;
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
    activateTab(tabId) {
        this.state.setActiveTab(tabId);
        this.renderTabs();
        this.renderEditor();
        // Update properties panel if it's open
        const propertiesPanel = document.getElementById('properties-panel');
        if (propertiesPanel && !propertiesPanel.classList.contains('hidden')) {
            this.renderProperties();
        }
    }
    closeTab(tabId) {
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
        this.renderTabs();
        this.renderEditor();
    }
    renderEditor() {
        // Cleanup existing CodeJar if any
        if (this.currentJar) {
            this.currentJar.destroy();
            this.currentJar = null;
        }
        this.currentQuill = null;
        const pane1 = document.getElementById('pane-1');
        if (!pane1)
            return;
        const content = pane1.querySelector('.editor-content');
        if (!content)
            return;
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
        if (!tab)
            return;
        // Ensure auto-save is running
        if (!tab.autoSaveInterval) {
            tab.autoSaveInterval = window.setInterval(() => {
                if (tab.isDirty) {
                    this.saveFile(tab.id);
                }
            }, 5000);
        }
        // Show/Hide mode selector based on file type
        const modeSelector = pane1.querySelector('.editor-mode-selector');
        if (modeSelector) {
            if (tab.fileType === 'markdown') {
                modeSelector.classList.remove('hidden');
                // Sync active button state
                modeSelector.querySelectorAll('.mode-btn').forEach(btn => {
                    if (btn.getAttribute('data-mode') === this.state.editorMode) {
                        btn.classList.add('active');
                    }
                    else {
                        btn.classList.remove('active');
                    }
                });
            }
            else {
                modeSelector.classList.add('hidden');
            }
        }
        // Handle different file types
        if (tab.fileType === 'image') {
            this.renderImageViewer(content, tab);
            return;
        }
        else if (tab.fileType === 'pdf') {
            this.renderPdfViewer(content, tab);
            return;
        }
        else if (tab.fileType === 'audio') {
            this.renderAudioViewer(content, tab);
            return;
        }
        else if (tab.fileType === 'video') {
            this.renderVideoViewer(content, tab);
            return;
        }
        else if (tab.fileType === 'other') {
            this.renderUnsupportedFile(content, tab);
            return;
        }
        else if (tab.fileType === 'text') {
            // For text/code files, always use raw editor
            this.renderRawEditor(content, tab);
            return;
        }
        // For markdown files, use the editor modes
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
    renderRawEditor(container, tab) {
        // If it's a code file (non-markdown text), show with syntax highlighting
        // We'll use a read-only view for now to support highlighting
        // Editing code with highlighting requires a more complex editor like Monaco or CodeMirror
        if (tab.fileType === 'text') {
            const ext = tab.fileName.split('.').pop() || 'txt';
            const language = ext === 'rs' ? 'rust' : (ext === 'ts' ? 'typescript' : (ext === 'js' ? 'javascript' : ext));
            container.innerHTML = `
                <div class="code-viewer">
                    <pre><code class="language-${language}">${this.escapeHtml(tab.content)}</code></pre>
                </div>
            `;
            container.querySelectorAll('pre code').forEach((block) => {
                hljs.highlightElement(block);
            });
            return;
        }
        container.innerHTML = `<textarea class="editor-raw" id="editor-textarea">${tab.content}</textarea>`;
        const textarea = container.querySelector('#editor-textarea');
        if (textarea) {
            textarea.addEventListener('input', () => {
                tab.content = textarea.value;
                tab.isDirty = true;
                this.renderTabs();
            });
        }
    }
    renderSideBySideEditor(container, tab) {
        container.innerHTML = `
            <div class="editor-side-by-side">
                <div>
                    <textarea class="editor-raw" id="editor-textarea">${tab.content}</textarea>
                </div>
                <div class="editor-preview markdown-content" id="preview-pane"></div>
            </div>
        `;
        const textarea = container.querySelector('#editor-textarea');
        const preview = container.querySelector('#preview-pane');
        if (textarea && preview) {
            const updatePreview = debounce(async () => {
                preview.innerHTML = await this.renderMarkdown(textarea.value);
            }, 300);
            textarea.addEventListener('input', () => {
                tab.content = textarea.value;
                tab.isDirty = true;
                updatePreview();
                this.renderTabs();
            });
            updatePreview();
        }
    }
    renderFormattedEditor(container, tab) {
        container.innerHTML = `<div class="editor-formatted language-markdown" id="editor-formatted"></div>`;
        const editor = container.querySelector('#editor-formatted');
        if (editor) {
            const jar = CodeJar(editor, (editor) => {
                hljs.highlightElement(editor);
            });
            jar.updateCode(tab.content, false);
            jar.onUpdate((code) => {
                tab.content = code;
                tab.isDirty = true;
                this.renderTabs();
            });
            this.currentJar = jar;
        }
    }
    async renderRenderedEditor(container, tab) {
        container.innerHTML = '<div class="loading">Loading WYSIWYG Editor...</div>';
        // Render markdown to HTML via backend
        const html = await this.renderMarkdown(tab.content);
        if (this.state.activeTabId !== tab.id)
            return;
        // Setup container
        container.innerHTML = `<div id="editor-wysiwyg" class="editor-wysiwyg"></div>`;
        const editorEl = container.querySelector('#editor-wysiwyg');
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
            // Track changes
            // @ts-ignore
            quill.on('text-change', debounce((delta, oldDelta, source) => {
                if (source !== 'user')
                    return;
                // @ts-ignore
                const newHtml = quill.root.innerHTML;
                const markdown = this.htmlToMarkdown(newHtml);
                tab.content = markdown;
                tab.isDirty = true;
                this.renderTabs();
            }, 500));
            this.currentQuill = quill;
        }
    }
    htmlToMarkdown(html) {
        // @ts-ignore
        const turndownService = new TurndownService({
            headingStyle: 'atx',
            codeBlockStyle: 'fenced'
        });
        // Add rule for wiki links (heuristic: internal links don't start with http)
        turndownService.addRule('wikiLink', {
            filter: function (node) {
                return node.nodeName === 'A' && node.getAttribute('href') && !node.getAttribute('href').startsWith('http');
            },
            replacement: function (content, node) {
                const href = node.getAttribute('href');
                if (href === content)
                    return `[[${href}]]`;
                return `[[${href}|${content}]]`;
            }
        });
        return turndownService.turndown(html);
    }
    renderImageViewer(container, tab) {
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
        const img = container.querySelector('#image-display');
        const zoomLevel = container.querySelector('#zoom-level');
        const imageContainer = container.querySelector('#image-container');
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
            if (!isPanning)
                return;
            e.preventDefault();
            const x = e.pageX - imageContainer.offsetLeft;
            const y = e.pageY - imageContainer.offsetTop;
            const walkX = (x - startX) * 2;
            const walkY = (y - startY) * 2;
            imageContainer.scrollLeft = scrollLeft - walkX;
            imageContainer.scrollTop = scrollTop - walkY;
        });
    }
    renderPdfViewer(container, tab) {
        container.innerHTML = `
            <div class="pdf-viewer">
                <iframe src="${tab.content}" width="100%" height="100%" style="border: none;"></iframe>
            </div>
        `;
    }
    renderAudioViewer(container, tab) {
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
    renderVideoViewer(container, tab) {
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
    renderUnsupportedFile(container, tab) {
        container.innerHTML = `
            <div class="unsupported-file">
                <h2>Unsupported File Type</h2>
                <p>This file type cannot be previewed in the browser.</p>
                <p><strong>File:</strong> ${tab.fileName}</p>
                <a href="${tab.content}" download="${tab.fileName}" class="btn btn-primary">Download File</a>
            </div>
        `;
    }
    async renderMarkdown(content) {
        try {
            return await this.api.renderMarkdown(content);
        }
        catch (e) {
            console.error('Markdown render error:', e);
            return `<p class="error">Failed to render markdown: ${e}</p>`;
        }
    }
    setupEventListeners() {
        // Vault selector
        const vaultSelect = document.getElementById('vault-select');
        vaultSelect?.addEventListener('change', (e) => {
            const target = e.target;
            this.switchVault(target.value);
        });
        // Add vault button
        const addVaultBtn = document.getElementById('add-vault-btn');
        addVaultBtn?.addEventListener('click', () => {
            this.showModal('add-vault-modal');
        });
        // Add vault form
        const addVaultForm = document.getElementById('add-vault-form');
        addVaultForm?.addEventListener('submit', async (e) => {
            e.preventDefault();
            const formData = new FormData(addVaultForm);
            const name = formData.get('name');
            const path = formData.get('path');
            try {
                await this.api.createVault(name, path);
                await this.loadVaults();
                this.hideModal('add-vault-modal');
                addVaultForm.reset();
            }
            catch (error) {
                alert('Failed to create vault: ' + error);
            }
        });
        // Modal close buttons
        document.querySelectorAll('[data-close-modal]').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const target = e.target;
                const modalId = target.getAttribute('data-close-modal');
                if (modalId)
                    this.hideModal(modalId);
            });
        });
        // Search
        const searchInput = document.getElementById('search-input');
        searchInput?.addEventListener('input', (e) => {
            if (this.state.searchDebounce) {
                clearTimeout(this.state.searchDebounce);
            }
            const target = e.target;
            const query = target.value.trim();
            if (query.length < 2)
                return;
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
                const target = e.target;
                const mode = target.getAttribute('data-mode');
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
            if (!tab)
                return;
            try {
                await this.api.downloadFile(this.state.currentVaultId, tab.filePath);
            }
            catch (error) {
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
            }
            catch (error) {
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
            }
            catch (error) {
                console.error('Failed to get daily note:', error);
                alert('Failed to get daily note: ' + error);
            }
        });
        // Upload functionality
        this.setupUploadHandlers();
        this.setupDragAndDrop();
    }
    setupUploadHandlers() {
        const uploadBtn = document.getElementById('upload-btn');
        const browseBtn = document.getElementById('browse-btn');
        const fileInput = document.getElementById('file-input');
        const uploadArea = document.getElementById('upload-area');
        uploadBtn?.addEventListener('click', () => {
            this.showModal('upload-modal');
        });
        browseBtn?.addEventListener('click', () => {
            fileInput?.click();
        });
        uploadArea?.addEventListener('click', (e) => {
            if (e.target === uploadArea || e.target.closest('.upload-prompt')) {
                fileInput?.click();
            }
        });
        fileInput?.addEventListener('change', (e) => {
            const files = e.target.files;
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
            if (!this.state.currentVaultId)
                return;
            const files = e.dataTransfer?.files;
            if (files && files.length > 0) {
                await this.handleUpload(files);
            }
        });
    }
    displaySelectedFiles(files) {
        const uploadList = document.getElementById('upload-list');
        if (!uploadList)
            return;
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
    async handleUpload(files) {
        if (!this.state.currentVaultId) {
            alert('Please select a vault first');
            return;
        }
        const progressContainer = document.getElementById('upload-progress');
        const progressBar = document.getElementById('progress-bar');
        const progressText = document.getElementById('progress-text');
        progressContainer?.classList.remove('hidden');
        try {
            await this.api.uploadFiles(this.state.currentVaultId, files, '', (loaded, total) => {
                const percentage = Math.round((loaded / total) * 100);
                if (progressBar)
                    progressBar.style.width = `${percentage}%`;
                if (progressText)
                    progressText.textContent = `Uploading... ${percentage}%`;
            });
            if (progressText)
                progressText.textContent = 'Upload complete!';
            setTimeout(() => {
                this.hideModal('upload-modal');
                progressContainer?.classList.add('hidden');
                this.loadFileTree();
            }, 1000);
        }
        catch (error) {
            console.error('Upload failed:', error);
            alert('Upload failed: ' + error);
            progressContainer?.classList.add('hidden');
        }
    }
    formatFileSize(bytes) {
        if (bytes === 0)
            return '0 Bytes';
        const k = 1024;
        const sizes = ['Bytes', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
    }
    togglePropertiesPanel() {
        const panel = document.getElementById('properties-panel');
        if (!panel)
            return;
        if (panel.classList.contains('hidden')) {
            this.showPropertiesPanel();
        }
        else {
            this.hidePropertiesPanel();
        }
    }
    showPropertiesPanel() {
        const panel = document.getElementById('properties-panel');
        if (!panel)
            return;
        panel.classList.remove('hidden');
        this.renderProperties();
    }
    hidePropertiesPanel() {
        const panel = document.getElementById('properties-panel');
        panel?.classList.add('hidden');
    }
    renderProperties() {
        const content = document.getElementById('properties-content');
        if (!content || !this.state.activeTabId)
            return;
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
    createPropertyItem(key, value) {
        const item = document.createElement('div');
        item.className = 'property-item';
        item.dataset.key = key;
        const valueType = Array.isArray(value) ? 'array' : typeof value;
        let valueStr = '';
        if (Array.isArray(value)) {
            valueStr = value.join(', ');
        }
        else if (typeof value === 'object' && value !== null) {
            valueStr = JSON.stringify(value);
        }
        else {
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
        if (!content)
            return;
        // Remove empty state if present
        const emptyState = content.querySelector('.empty-state');
        if (emptyState) {
            emptyState.remove();
        }
        const propertyItem = this.createPropertyItem('', '');
        content.appendChild(propertyItem);
        // Focus the key input
        const keyInput = propertyItem.querySelector('.property-key');
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
        const properties = {};
        const propertyItems = document.querySelectorAll('.property-item');
        for (const item of Array.from(propertyItems)) {
            const keyInput = item.querySelector('.property-key');
            const valueTextarea = item.querySelector('.property-value');
            const typeSelect = item.querySelector('.property-type');
            const key = keyInput.value.trim();
            const valueStr = valueTextarea.value.trim();
            const type = typeSelect.value;
            if (!key)
                continue; // Skip empty keys
            let value;
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
            await this.api.writeFile(this.state.currentVaultId, tab.filePath, tab.content, tab.modified, tab.frontmatter);
            tab.isDirty = false;
            this.renderTabs();
            alert('Properties saved successfully');
        }
        catch (error) {
            console.error('Failed to save properties:', error);
            alert('Failed to save properties: ' + error);
        }
    }
    async performSearch(query) {
        if (!this.state.currentVaultId)
            return;
        try {
            const results = await this.api.search(this.state.currentVaultId, query);
            this.renderSearchResults(results);
            this.showModal('search-modal');
        }
        catch (error) {
            console.error('Search failed:', error);
        }
    }
    renderSearchResults(results) {
        const container = document.getElementById('search-results');
        if (!container)
            return;
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
    highlightMatch(text, start, end) {
        return text.substring(0, start) +
            '<mark>' + text.substring(start, end) + '</mark>' +
            text.substring(end);
    }
    escapeHtml(unsafe) {
        return unsafe
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#039;");
    }
    showModal(modalId) {
        const modal = document.getElementById(modalId);
        modal?.classList.remove('hidden');
    }
    hideModal(modalId) {
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
                if (!this.state.currentVaultId || changeEvent.vault_id !== this.state.currentVaultId)
                    return;
                const { event_type, path } = changeEvent;
                // Handle file tree updates
                if (event_type === 'Created' || event_type === 'Deleted' || 'Renamed' in event_type) {
                    await this.loadFileTree();
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
                            }
                            else {
                                // Notify user of conflict?
                                console.warn('External modification on dirty file:', path);
                            }
                        }
                        else if (event_type === 'Deleted') {
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
                        const renamedEvent = event_type;
                        if (renamedEvent.from === tab.filePath) {
                            tab.filePath = renamedEvent.to;
                            tab.fileName = renamedEvent.to.split('/').pop() || renamedEvent.to;
                            this.renderTabs();
                        }
                    }
                }
            }
            catch (error) {
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
            const delay = Math.min(1000 * Math.pow(2, this.state.wsReconnectAttempts - 1), this.state.wsMaxReconnectDelay);
            console.log(`Reconnecting in ${delay / 1000}s (attempt ${this.state.wsReconnectAttempts})...`);
            this.updateConnectionStatus('reconnecting', delay);
            this.state.wsReconnectTimeout = window.setTimeout(() => {
                this.setupWebSocket();
            }, delay);
        };
        this.state.ws = ws;
    }
    updateConnectionStatus(status, delay) {
        // Update UI to show connection status
        // This could be a status indicator in the header
        const statusElement = document.getElementById('connection-status');
        if (!statusElement)
            return;
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
        const input = document.getElementById('quick-switcher-input');
        const resultsContainer = document.getElementById('quick-switcher-results');
        // Global keyboard shortcut (Ctrl+O or Cmd+O not working well in browser, usually opens files)
        // Using Ctrl+K or Cmd+K
        document.addEventListener('keydown', (e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
                e.preventDefault();
                this.showModal('quick-switcher-modal');
                input?.focus();
                input.value = '';
                if (resultsContainer)
                    resultsContainer.innerHTML = '';
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
            const query = e.target.value;
            if (this.state.quickSwitcherDebounce) {
                clearTimeout(this.state.quickSwitcherDebounce);
            }
            this.state.quickSwitcherDebounce = window.setTimeout(() => {
                this.performQuickSwitcherSearch(query);
            }, 200);
        });
        // Keyboard navigation in list
        input?.addEventListener('keydown', (e) => {
            if (!resultsContainer)
                return;
            const items = resultsContainer.querySelectorAll('.search-result-item');
            const activeItem = resultsContainer.querySelector('.search-result-item.active');
            let index = Array.from(items).indexOf(activeItem);
            if (e.key === 'ArrowDown') {
                e.preventDefault();
                index = index < items.length - 1 ? index + 1 : 0;
                this.highlightQuickSwitcherItem(items, index);
            }
            else if (e.key === 'ArrowUp') {
                e.preventDefault();
                index = index > 0 ? index - 1 : items.length - 1;
                this.highlightQuickSwitcherItem(items, index);
            }
            else if (e.key === 'Enter') {
                e.preventDefault();
                if (activeItem) {
                    activeItem.click();
                }
                else if (items.length > 0) {
                    // Default to first item if none active
                    items[0].click();
                }
            }
        });
    }
    highlightQuickSwitcherItem(items, index) {
        items.forEach(item => item.classList.remove('active'));
        if (items[index]) {
            items[index].classList.add('active');
            items[index].scrollIntoView({ block: 'nearest' });
        }
    }
    async performQuickSwitcherSearch(query) {
        if (!this.state.currentVaultId)
            return;
        try {
            // Reuse the search API but with a different limit or params if needed
            // For quick switcher, we mostly care about file paths/names
            // If query is empty, maybe show recent files? For now just show nothing or all files
            let results = [];
            if (query.trim() === '') {
                // Show recent files
                results = this.state.recentFiles.map(path => ({
                    title: path.split('/').pop() || path,
                    path: path,
                    score: 0,
                    matches: []
                }));
            }
            else {
                results = await this.api.search(this.state.currentVaultId, query, 20);
            }
            this.renderQuickSwitcherResults(results);
        }
        catch (error) {
            console.error('Quick switcher search failed:', error);
        }
    }
    renderQuickSwitcherResults(results) {
        const container = document.getElementById('quick-switcher-results');
        if (!container)
            return;
        if (results.length === 0) {
            container.innerHTML = '<div class="empty-state"><p>No matching files</p></div>';
            return;
        }
        container.innerHTML = '';
        results.forEach((result, index) => {
            const item = document.createElement('div');
            item.className = 'search-result-item'; // Reusing search styles for now
            if (index === 0)
                item.classList.add('active');
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
        if (!this.state.currentVaultId)
            return;
        const listContainer = document.getElementById('template-list');
        if (!listContainer)
            return;
        listContainer.innerHTML = '<p>Loading templates...</p>';
        try {
            // Find "Templates" folder
            // We'll search for a directory named "Templates" (case insensitive)
            // Implementation detail: we fetch file tree and look for it
            const tree = await this.api.getFileTree(this.state.currentVaultId);
            // Helper to find folder
            const findTemplatesFolder = (nodes) => {
                for (const node of nodes) {
                    if (node.is_directory && node.name.toLowerCase() === 'templates') {
                        return node;
                    }
                    if (node.children) {
                        const found = findTemplatesFolder(node.children);
                        if (found)
                            return found;
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
        }
        catch (error) {
            console.error('Failed to load templates:', error);
            listContainer.innerHTML = `<p class="error">Failed to load templates: ${error}</p>`;
        }
    }
    renderTemplates(nodes) {
        const listContainer = document.getElementById('template-list');
        if (!listContainer)
            return;
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
    async insertTemplate(templatePath) {
        if (!this.state.currentVaultId || !this.state.activeTabId)
            return;
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
            }
            else {
                console.warn('Could not find textarea to insert template');
                alert('Could not insert template: Editor not found or not in raw mode.');
            }
        }
        catch (error) {
            console.error('Failed to insert template:', error);
            alert('Failed to insert template: ' + error);
        }
    }
    applyTemplateVariables(content) {
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
        const tab = this.state.getTab(this.state.activeTabId);
        if (tab) {
            const title = tab.fileName.replace(/\.md$/, '');
            processed = processed.replace(/{{title}}/g, title);
        }
        return processed;
    }
    async createDefaultTemplates() {
        if (!this.state.currentVaultId)
            return;
        try {
            // Create Templates directory
            // We use a try/catch here in case it already exists (api might error or succeed depending on implementation)
            try {
                await this.api.createDirectory(this.state.currentVaultId, 'Templates');
            }
            catch (e) {
                // Ignore if it already exists or handle specifically
                if (!e.toString().includes('already exists')) {
                    // console.warn('Directory creation warning:', e);
                }
            }
            // Create Daily Note Template
            await this.api.createFile(this.state.currentVaultId, 'Templates/Daily Note.md', '# {{date}}\n\n## Tasks\n- [ ] \n\n## Notes\n');
            // Create Meeting Note Template
            await this.api.createFile(this.state.currentVaultId, 'Templates/Meeting Note.md', '# {{title}}\nDate: {{datetime}}\n\n## Attendees\n\n## Agenda\n\n## Notes\n');
            // Reload templates
            await this.loadTemplates();
            alert('Default templates created successfully.');
        }
        catch (error) {
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
            if (!this.state.conflictData)
                return;
            console.log('User chose to keep their version');
            this.hideModal('conflict-modal');
            alert('Your changes will be saved (save functionality to be implemented)');
        });
        useServerBtn?.addEventListener('click', async () => {
            if (!this.state.conflictData || !this.state.currentVaultId)
                return;
            try {
                const fileData = await this.api.readFile(this.state.currentVaultId, this.state.conflictData.filePath);
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
            }
            catch (error) {
                console.error('Failed to load server version:', error);
                alert('Failed to load server version: ' + error);
            }
        });
        viewBothBtn?.addEventListener('click', () => {
            if (!this.state.conflictData)
                return;
            console.log('View both versions');
            alert('Side-by-side comparison view (to be implemented)');
        });
        cancelBtn?.addEventListener('click', () => {
            this.hideModal('conflict-modal');
            this.state.conflictData = null;
        });
    }
    showConflictResolution(filePath, yourVersion, serverVersion) {
        this.state.conflictData = { filePath, yourVersion, serverVersion };
        const fileNameEl = document.getElementById('conflict-file-name');
        const yourVersionEl = document.getElementById('conflict-your-version');
        const serverVersionEl = document.getElementById('conflict-server-version');
        if (fileNameEl)
            fileNameEl.textContent = filePath;
        if (yourVersionEl)
            yourVersionEl.textContent = yourVersion.substring(0, 500) + (yourVersion.length > 500 ? '...' : '');
        if (serverVersionEl)
            serverVersionEl.textContent = serverVersion.substring(0, 500) + (serverVersion.length > 500 ? '...' : '');
        this.showModal('conflict-modal');
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
    ui.setupQuickSwitcher();
    ui.setupTemplates();
    ui.setupConflictResolution();
});
