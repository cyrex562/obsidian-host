# Obsidian Host Plugins

This directory contains core plugins for Obsidian Host. Each plugin demonstrates different aspects of the plugin API.

## Core Plugins

### 1. Daily Notes (`daily-notes/`)
Automatically creates and manages daily notes with template support.

**Features:**
- Create daily notes with customizable date format
- Template support with variable substitution
- Commands for opening today, yesterday, and tomorrow's notes
- Automatic opening on startup (configurable)
- Ribbon icon for quick access

**Capabilities:**
- `read_files` - Read template files
- `write_files` - Create daily notes
- `commands` - Register commands
- `storage` - Store configuration
- `modify_ui` - Add ribbon icon

**Configuration:**
```json
{
  "daily_notes_folder": "Daily Notes",
  "date_format": "YYYY-MM-DD",
  "template_file": "Templates/Daily Note.md",
  "open_on_startup": true
}
```

### 2. Word Count (`word-count/`)
Displays real-time statistics about the current document.

**Features:**
- Word count
- Character count (excluding spaces)
- Reading time estimation
- Status bar integration
- Updates on editor changes

**Capabilities:**
- `editor_access` - Read editor content
- `modify_ui` - Add status bar item

**Configuration:**
```json
{
  "show_word_count": true,
  "show_char_count": true,
  "show_reading_time": true,
  "words_per_minute": 200
}
```

### 3. Backlinks (`backlinks/`)
Shows all notes that link to the current note.

**Features:**
- Wiki link detection
- Unlinked mentions (optional)
- Real-time index updates
- Case-sensitive/insensitive matching

**Capabilities:**
- `read_files` - Scan all notes
- `vault_metadata` - Access vault information
- `modify_ui` - Display backlinks panel

**Configuration:**
```json
{
  "show_unlinked_mentions": true,
  "case_sensitive": false
}
```

## Plugin Structure

Each plugin follows this structure:

```
plugin-name/
├── manifest.json    # Plugin metadata and configuration
├── main.js          # Plugin implementation
└── README.md        # Plugin documentation (optional)
```

### manifest.json

Required fields:
- `id` - Unique identifier (reverse domain notation)
- `name` - Human-readable name
- `version` - Semantic version
- `main` - Entry point file
- `capabilities` - Required permissions

Optional fields:
- `description` - Short description
- `author` - Author name
- `license` - License identifier
- `plugin_type` - "javascript" or "wasm"
- `styles` - CSS files to load
- `min_host_version` - Minimum host version
- `dependencies` - Plugin dependencies
- `hooks` - Lifecycle hooks
- `config_schema` - JSON Schema for configuration

### main.js

Plugin class with lifecycle methods:

```javascript
class MyPlugin {
  constructor(api) {
    this.api = api;
  }

  async onLoad(ctx) {
    // Plugin initialization
  }

  async onUnload() {
    // Cleanup
  }

  // Optional lifecycle hooks
  async onStartup() { }
  async onFileOpen(ctx, filePath) { }
  async onFileSave(ctx, filePath) { }
  async onEditorChange(ctx, content) { }
}

export default MyPlugin;
```

## Plugin API

Plugins have access to the Plugin API through `this.api`:

### File Operations
```javascript
await this.api.read_file(vaultId, filePath);
await this.api.write_file(vaultId, filePath, content);
await this.api.delete_file(vaultId, filePath);
await this.api.list_files(vaultId, pattern);
```

### Storage
```javascript
await this.api.storage_get(key);
await this.api.storage_set(key, value);
await this.api.storage_delete(key);
await this.api.storage_clear();
```

### Events
```javascript
await this.api.on_event(eventType, callback);
await this.api.emit_event(event);
await this.api.off_event(subscriptionId);
```

### UI
```javascript
await this.api.register_command(command);
await this.api.show_notice(message, duration);
this.api.addRibbonIcon(icon, tooltip, callback);
this.api.addStatusBarItem();
```

### Utilities
```javascript
await this.api.parse_markdown(markdown);
await this.api.extract_frontmatter(content);
```

## Development

### Creating a New Plugin

1. Create a new directory in `plugins/`
2. Add `manifest.json` with plugin metadata
3. Create `main.js` with plugin class
4. Export the plugin class as default
5. Test with Obsidian Host

### Testing

Plugins are automatically discovered and loaded from this directory. Enable/disable plugins through the plugin management UI.

### Best Practices

1. **Minimal Capabilities**: Request only necessary permissions
2. **Error Handling**: Handle all errors gracefully
3. **Cleanup**: Properly cleanup in `onUnload`
4. **Performance**: Avoid blocking operations
5. **Configuration**: Use config schema for user settings
6. **Documentation**: Include clear documentation

## Security

Plugins run with explicit capabilities. Users must approve permissions on installation. Plugins cannot:
- Access files outside the vault
- Make network requests without permission
- Execute system commands without permission
- Access other plugins' data

## Future Plugins

Planned core plugins:
- Templates - Template insertion and management
- Tag Browser - Browse and manage tags
- Outline/TOC - Table of contents generation
- Graph View - Visualize note connections
- Search - Advanced search functionality

## Contributing

To contribute a plugin:
1. Follow the plugin structure
2. Include comprehensive documentation
3. Test thoroughly
4. Submit a pull request

## License

Core plugins are licensed under MIT License.
