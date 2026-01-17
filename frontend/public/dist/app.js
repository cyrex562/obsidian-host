"use strict";
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
    if (['txt', 'json', 'js', 'ts', 'css', 'html', 'xml'].includes(ext))
        return 'text';
    return 'other';
}
function isImageFile(filePath) {
    return getFileType(filePath) === 'image';
}
// App State
class AppState {
    constructor() {
        this.currentVaultId = null;
        this.vaults = [];
        this.openTabs = new Map();
        this.activeTabId = null;
        this.editorMode = 'raw';
        this.ws = null;
        this.searchDebounce = null;
    }
    setVault(vaultId) {
        this.currentVaultId = vaultId;
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
    async writeFile(vaultId, filePath, content, lastModified) {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/files/${filePath}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ content, last_modified: lastModified }),
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
    async search(vaultId, query, limit = 50) {
        const response = await fetch(`${this.baseUrl}/api/vaults/${vaultId}/search?q=${encodeURIComponent(query)}&limit=${limit}`);
        if (!response.ok)
            throw new Error('Failed to search');
        return response.json();
    }
}
// UI Manager
class UIManager {
    constructor(state, api) {
        this.state = state;
        this.api = api;
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
            container.appendChild(item);
            if (node.is_directory && node.children && node.children.length > 0) {
                const childContainer = document.createElement('div');
                childContainer.className = 'file-tree-children';
                container.appendChild(childContainer);
                this.renderFileTree(node.children, childContainer);
            }
        }
    }
    async openFile(filePath) {
        if (!this.state.currentVaultId)
            return;
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
            // For text-based files, read the content
            if (fileType === 'markdown' || fileType === 'text') {
                const fileContent = await this.api.readFile(this.state.currentVaultId, filePath);
                content = fileContent.content;
                modified = fileContent.modified;
            }
            else if (fileType === 'image' || fileType === 'pdf') {
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
        try {
            const updated = await this.api.writeFile(this.state.currentVaultId, tab.filePath, tab.content, tab.modified);
            tab.modified = updated.modified;
            tab.isDirty = false;
            this.renderTabs();
        }
        catch (error) {
            console.error('Failed to save file:', error);
            alert('Failed to save file: ' + error);
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
    }
    closeTab(tabId) {
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
        // Handle different file types
        if (tab.fileType === 'image') {
            this.renderImageViewer(content, tab);
            return;
        }
        else if (tab.fileType === 'pdf') {
            this.renderPdfViewer(content, tab);
            return;
        }
        else if (tab.fileType === 'other') {
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
    renderRawEditor(container, tab) {
        container.innerHTML = `<textarea class="editor-raw" id="editor-textarea">${tab.content}</textarea>`;
        const textarea = container.querySelector('#editor-textarea');
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
    renderFormattedEditor(container, tab) {
        // Similar to raw but with syntax highlighting (simplified for now)
        this.renderRawEditor(container, tab);
    }
    renderRenderedEditor(container, tab) {
        container.innerHTML = `<div class="markdown-content">${this.renderMarkdown(tab.content)}</div>`;
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
    renderMarkdown(content) {
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
        ws.onmessage = (event) => {
            try {
                const changeEvent = JSON.parse(event.data);
                console.log('File change event:', changeEvent);
                // Reload file tree if current vault
                if (changeEvent.vault_id === this.state.currentVaultId) {
                    this.loadFileTree();
                }
            }
            catch (error) {
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
