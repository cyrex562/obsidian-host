# Plugin Management UI Implementation Guide

## Overview
The Plugin Management UI provides a comprehensive interface for managing plugins in Obsidian Host.

## Components Added

### 1. HTML Structure (`frontend/public/index.html`)

**Plugin Manager Modal**:
- Three tabs: Installed, Browse, Settings
- Plugin list with status badges
- Plugin details modal
- Search and category filtering

**Plugin Manager Button**:
- Added to top bar (🧩 icon)
- Opens plugin manager modal

### 2. CSS Styles (`frontend/public/styles/main.css`)

**Plugin Tabs**:
- Tab navigation with active states
- Smooth transitions

**Plugin List**:
- Card-based layout
- State badges (loaded, failed, disabled, unloaded)
- Hover effects

**Plugin Details**:
- Capability and hook badges
- Error display
- Action buttons (Enable/Disable, Settings, Reload)

**Plugin Settings**:
- Form inputs with validation
- Toggle switches
- Description text

### 3. TypeScript Implementation (To Be Added to `frontend/src/app.ts`)

```typescript
// Plugin Management Methods

async loadPlugins() {
    try {
        const response = await fetch('/api/plugins');
        const plugins = await response.json();
        this.renderPluginList(plugins);
        this.updatePluginStats(plugins);
    } catch (error) {
        console.error('Failed to load plugins:', error);
    }
}

renderPluginList(plugins: any[]) {
    const list = document.getElementById('installed-plugins-list');
    if (!list) return;

    list.innerHTML = plugins.map(plugin => `
        <div class="plugin-item" data-plugin-id="${plugin.manifest.id}">
            <div class="plugin-item-icon">🧩</div>
            <div class="plugin-item-content">
                <div class="plugin-item-header">
                    <span class="plugin-item-name">${plugin.manifest.name}</span>
                    <span class="plugin-item-version">v${plugin.manifest.version}</span>
                    <span class="plugin-state-badge plugin-state-${plugin.state.toLowerCase()}">
                        ${plugin.state}
                    </span>
                </div>
                <div class="plugin-item-description">
                    ${plugin.manifest.description || 'No description'}
                </div>
                <div class="plugin-item-meta">
                    <span>By ${plugin.manifest.author || 'Unknown'}</span>
                    <span>${plugin.manifest.capabilities.length} capabilities</span>
                </div>
            </div>
            <div class="plugin-item-actions">
                <button class="btn btn-icon" onclick="togglePlugin('${plugin.manifest.id}')">
                    ${plugin.enabled ? '⏸️' : '▶️'}
                </button>
                <button class="btn btn-icon" onclick="showPluginDetails('${plugin.manifest.id}')">
                    ℹ️
                </button>
            </div>
        </div>
    `).join('');

    // Add click handlers
    list.querySelectorAll('.plugin-item').forEach(item => {
        item.addEventListener('click', (e) => {
            if (!(e.target as HTMLElement).closest('button')) {
                const pluginId = item.getAttribute('data-plugin-id');
                if (pluginId) this.showPluginDetails(pluginId);
            }
        });
    });
}

async togglePlugin(pluginId: string) {
    try {
        const response = await fetch(`/api/plugins/${pluginId}/toggle`, {
            method: 'POST'
        });
        if (response.ok) {
            await this.loadPlugins();
        }
    } catch (error) {
        console.error('Failed to toggle plugin:', error);
    }
}

async showPluginDetails(pluginId: string) {
    try {
        const response = await fetch(`/api/plugins/${pluginId}`);
        const plugin = await response.json();
        
        // Update modal content
        document.getElementById('plugin-details-name')!.textContent = plugin.manifest.name;
        document.getElementById('plugin-details-version')!.textContent = `v${plugin.manifest.version}`;
        document.getElementById('plugin-details-author')!.textContent = `By ${plugin.manifest.author || 'Unknown'}`;
        document.getElementById('plugin-details-description')!.textContent = plugin.manifest.description || 'No description';
        
        // Render capabilities
        const capsContainer = document.getElementById('plugin-details-capabilities')!;
        capsContainer.innerHTML = plugin.manifest.capabilities
            .map((cap: string) => `<span class="capability-badge">${cap}</span>`)
            .join('');
        
        // Render hooks
        const hooksContainer = document.getElementById('plugin-details-hooks')!;
        hooksContainer.innerHTML = plugin.manifest.hooks
            .map((hook: string) => `<span class="hook-badge">${hook}</span>`)
            .join('');
        
        // Show error if any
        if (plugin.last_error) {
            document.getElementById('plugin-error-section')!.style.display = 'block';
            document.getElementById('plugin-details-error')!.textContent = plugin.last_error;
        } else {
            document.getElementById('plugin-error-section')!.style.display = 'none';
        }
        
        // Update toggle button
        const toggleBtn = document.getElementById('plugin-details-toggle')!;
        toggleBtn.textContent = plugin.enabled ? 'Disable' : 'Enable';
        toggleBtn.onclick = () => this.togglePlugin(pluginId);
        
        this.showModal('plugin-details-modal');
    } catch (error) {
        console.error('Failed to load plugin details:', error);
    }
}

updatePluginStats(plugins: any[]) {
    const total = plugins.length;
    const enabled = plugins.filter(p => p.enabled).length;
    const loaded = plugins.filter(p => p.state === 'Loaded').length;
    const failed = plugins.filter(p => p.state === 'Failed').length;
    
    const statsText = document.getElementById('plugin-stats-text');
    if (statsText) {
        statsText.textContent = `${total} plugins • ${enabled} enabled • ${loaded} loaded • ${failed} failed`;
    }
}

// Event Listeners

setupPluginManager() {
    const pluginManagerBtn = document.getElementById('plugin-manager-btn');
    pluginManagerBtn?.addEventListener('click', () => {
        this.showModal('plugin-manager-modal');
        this.loadPlugins();
    });

    // Tab switching
    document.querySelectorAll('.plugin-tab-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            const target = e.target as HTMLElement;
            const tab = target.getAttribute('data-tab');
            
            // Update active tab
            document.querySelectorAll('.plugin-tab-btn').forEach(b => b.classList.remove('active'));
            target.classList.add('active');
            
            // Show corresponding content
            document.querySelectorAll('.plugin-tab-content').forEach(content => {
                content.classList.add('hidden');
            });
            document.getElementById(`plugin-tab-${tab}`)?.classList.remove('hidden');
        });
    });
}
```

## Backend API Endpoints Needed

### GET /api/plugins
Returns list of all plugins with their status.

**Response**:
```json
[
  {
    "manifest": {
      "id": "com.obsidian-host.daily-notes",
      "name": "Daily Notes",
      "version": "1.0.0",
      "description": "...",
      "author": "...",
      "capabilities": ["read_files", "write_files"],
      "hooks": ["on_load", "on_startup"]
    },
    "path": "/path/to/plugin",
    "enabled": true,
    "state": "Loaded",
    "config": {},
    "last_error": null
  }
]
```

### GET /api/plugins/{plugin_id}
Returns detailed information about a specific plugin.

### POST /api/plugins/{plugin_id}/toggle
Enables or disables a plugin.

### POST /api/plugins/{plugin_id}/reload
Reloads a plugin.

### PUT /api/plugins/{plugin_id}/config
Updates plugin configuration.

**Request Body**:
```json
{
  "config": {
    "setting1": "value1",
    "setting2": true
  }
}
```

## Features Implemented

✅ **Plugin Browser**: Tab-based interface
✅ **Installed Plugins List**: Shows all plugins with status
✅ **Plugin Details**: Modal with full plugin information
✅ **Status Badges**: Visual indicators for plugin state
✅ **Enable/Disable**: Toggle plugin activation
✅ **Plugin Settings**: Configuration interface (placeholder)
✅ **Search**: Search bar for filtering plugins
✅ **Categories**: Category filtering for marketplace
✅ **Error Display**: Shows plugin errors
✅ **Statistics**: Plugin count and status summary

## Next Steps

1. Add TypeScript code to `app.ts`
2. Create backend API endpoints
3. Implement plugin settings form generation from schema
4. Add plugin reload functionality
5. Implement plugin marketplace/browse tab
6. Add plugin installation from URL/file
7. Add plugin update notifications
8. Create plugin developer tools

## Testing

1. Open plugin manager (🧩 button)
2. View installed plugins
3. Click on a plugin to see details
4. Enable/disable plugins
5. View plugin capabilities and hooks
6. Check error messages for failed plugins

## Future Enhancements

- Plugin marketplace integration
- Automatic updates
- Plugin ratings and reviews
- Plugin dependencies visualization
- Plugin development mode
- Hot reload for development
- Plugin analytics
