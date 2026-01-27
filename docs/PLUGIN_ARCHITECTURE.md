# Plugin Architecture Documentation

## Overview

The Obsidian Host plugin system provides a secure, extensible way to add functionality to the application. Plugins can be written in JavaScript or WebAssembly (WASM) and run in a controlled environment with explicit permissions.

## Plugin Manifest Format

Every plugin must include a `manifest.json` file in its root directory:

```json
{
  "id": "com.example.myplugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "description": "A sample plugin that demonstrates the plugin API",
  "author": "John Doe",
  "license": "MIT",
  "main": "main.js",
  "plugin_type": "javascript",
  "styles": ["styles.css"],
  "min_host_version": "0.1.0",
  "dependencies": {
    "com.example.otherplugin": "^1.0.0"
  },
  "capabilities": [
    "read_files",
    "write_files",
    "modify_ui",
    "commands"
  ],
  "hooks": [
    "on_load",
    "on_file_open",
    "on_file_save"
  ],
  "config_schema": {
    "type": "object",
    "properties": {
      "enabled_features": {
        "type": "array",
        "items": { "type": "string" }
      }
    }
  }
}
```

### Manifest Fields

- **id** (required): Unique identifier using reverse domain notation
- **name** (required): Human-readable plugin name
- **version** (required): Semantic version (MAJOR.MINOR.PATCH)
- **description**: Short description of functionality
- **author**: Plugin author name/email
- **license**: SPDX license identifier
- **main** (required): Entry point file (main.js or plugin.wasm)
- **plugin_type**: "javascript" or "wasm" (default: "javascript")
- **styles**: Array of CSS files to load
- **min_host_version**: Minimum required host version
- **dependencies**: Map of plugin_id to version requirement
- **capabilities**: Array of required permissions
- **hooks**: Array of lifecycle hooks to implement
- **config_schema**: JSON Schema for plugin configuration

## Plugin Types

### JavaScript Plugins

JavaScript plugins run in a sandboxed environment with access to the Plugin API.

**Entry Point (main.js):**
```javascript
export default class MyPlugin {
  async onLoad(ctx) {
    console.log('Plugin loaded!', ctx);
    // Register commands, UI elements, etc.
  }

  async onUnload() {
    console.log('Plugin unloaded!');
    // Cleanup
  }

  async onFileOpen(ctx, filePath) {
    console.log('File opened:', filePath);
  }
}
```

### WASM Plugins

WASM plugins provide better performance and security isolation.

**Entry Point (plugin.wasm):**
Compiled from Rust, C++, or other languages that target WebAssembly.

## Capabilities (Permissions)

Plugins must declare required capabilities in their manifest. Users approve these on installation.

### Available Capabilities

- **read_files**: Read files from the vault
- **write_files**: Create/modify files in the vault
- **delete_files**: Delete files from the vault
- **vault_metadata**: Access vault information
- **network**: Make HTTP/HTTPS requests
- **storage**: Access local storage/cache
- **modify_ui**: Add UI elements (ribbons, status bar, etc.)
- **commands**: Register commands in command palette
- **editor_access**: Read/modify editor content
- **system_exec**: Execute system commands (highly restricted)

### Security Model

1. **Explicit Permissions**: Plugins must declare all capabilities
2. **User Approval**: Users approve capabilities on installation
3. **Sandboxing**: Plugins run in isolated environments
4. **API-Only Access**: No direct file system or network access
5. **Capability Revocation**: Users can revoke permissions anytime

## Lifecycle Hooks

Plugins can implement hooks to respond to application events:

### Core Hooks

- **on_load**: Called when plugin is loaded
- **on_unload**: Called when plugin is unloaded
- **on_startup**: Called on application startup
- **on_shutdown**: Called on application shutdown

### File Hooks

- **on_file_open**: Called when a file is opened
- **on_file_save**: Called when a file is saved
- **on_file_create**: Called when a file is created
- **on_file_delete**: Called when a file is deleted
- **on_file_rename**: Called when a file is renamed

### Editor Hooks

- **on_editor_change**: Called when editor content changes
- **on_vault_switch**: Called when active vault changes

## Plugin API Surface

### File Operations

```javascript
// Read file content
const content = await api.readFile(filePath);

// Write file content
await api.writeFile(filePath, content);

// Delete file
await api.deleteFile(filePath);

// List files
const files = await api.listFiles(pattern);
```

### Vault Operations

```javascript
// Get current vault
const vault = await api.getCurrentVault();

// Get vault metadata
const metadata = await api.getVaultMetadata(vaultId);
```

### UI Operations

```javascript
// Add ribbon icon
api.addRibbonIcon('icon-id', 'Tooltip', () => {
  console.log('Clicked!');
});

// Add status bar item
const statusBar = api.addStatusBarItem();
statusBar.setText('Status');

// Show notification
api.showNotice('Hello from plugin!');
```

### Command Registration

```javascript
// Register command
api.addCommand({
  id: 'my-command',
  name: 'My Command',
  callback: () => {
    console.log('Command executed!');
  },
  hotkey: 'Ctrl+Shift+M'
});
```

### Storage Operations

```javascript
// Save plugin data
await api.storage.set('key', { data: 'value' });

// Load plugin data
const data = await api.storage.get('key');
```

## Plugin States

Plugins transition through these states:

1. **Unloaded**: Plugin discovered but not loaded
2. **Loading**: Plugin is being loaded
3. **Loaded**: Plugin is active and running
4. **Failed**: Plugin failed to load (error in last_error)
5. **Disabled**: Plugin disabled by user

## Dependency Resolution

Plugins can depend on other plugins:

```json
{
  "dependencies": {
    "com.example.base-plugin": "^1.0.0",
    "com.example.utils": "~2.1.0"
  }
}
```

Version requirements follow npm semver syntax:
- `^1.0.0`: Compatible with 1.x.x
- `~1.2.0`: Compatible with 1.2.x
- `1.2.3`: Exact version
- `>=1.0.0 <2.0.0`: Range

## Plugin Discovery

Plugins are discovered by scanning the `plugins/` directory:

```
plugins/
├── my-plugin/
│   ├── manifest.json
│   ├── main.js
│   └── styles.css
└── another-plugin/
    ├── manifest.json
    └── plugin.wasm
```

## Configuration

Plugins can define a configuration schema:

```json
{
  "config_schema": {
    "type": "object",
    "properties": {
      "api_key": {
        "type": "string",
        "description": "API key for external service"
      },
      "max_items": {
        "type": "integer",
        "default": 10,
        "minimum": 1,
        "maximum": 100
      }
    },
    "required": ["api_key"]
  }
}
```

User configuration is validated against this schema and passed to the plugin.

## Best Practices

1. **Minimal Capabilities**: Request only necessary permissions
2. **Error Handling**: Handle all errors gracefully
3. **Cleanup**: Properly cleanup in onUnload
4. **Performance**: Avoid blocking operations
5. **Documentation**: Include README.md with usage instructions
6. **Versioning**: Follow semantic versioning
7. **Testing**: Test with different vault configurations

## Example Plugin

See `plugins/example-plugin/` for a complete working example.

## Security Considerations

1. **Input Validation**: Validate all user input
2. **XSS Prevention**: Sanitize HTML content
3. **Path Traversal**: Use API methods, not direct file access
4. **Network Security**: Validate external data
5. **Secrets Management**: Never hardcode secrets
6. **User Privacy**: Respect user data privacy

## Future Enhancements

- Plugin marketplace/registry
- Automatic updates
- Plugin sandboxing with WASM
- Hot reload during development
- Plugin analytics and telemetry
- Community plugin repository
