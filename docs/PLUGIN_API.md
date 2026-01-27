# Plugin API Documentation

Obsidian Host supports a frontend-centric plugin system allowing you to extend the UI and functionality using JavaScript and CSS.

## Plugin Structure

A plugin is a folder containing at least a `manifest.json`.

```
plugins/
  my-plugin/
    manifest.json
    main.js
    styles.css (optional)
```

### `manifest.json`

```json
{
    "id": "my-plugin",
    "name": "My Plugin",
    "version": "1.0.0",
    "description": "Short description",
    "author": "Your Name",
    "main": "main.js",
    "styles": ["styles.css"]
}
```

## JavaScript API

Plugins run in the context of the browser window. A global `window.app` object is exposed (planned) or plugins can interact with the DOM directly.

### Lifecycle

-   **Load**: The plugin's `main.js` is loaded as a standard script tag or ES module when the app starts.

### Sandbox

Currently, plugins run with **full access** to the `window` object. There is no strict sandboxing yet.

## Events

Plugins can listen to standard DOM events or custom app events.

```javascript
document.addEventListener('app-file-open', (e) => {
    console.log('Opened file:', e.detail.path);
});
```

## Example

```javascript
// main.js
console.log("My Plugin Loaded");

// Create a status bar item
const statusBar = document.querySelector('.status-bar');
if (statusBar) {
    const item = document.createElement('div');
    item.textContent = 'Plugin Active';
    statusBar.appendChild(item);
}
```

## Security Warning

Plugins have full access to the user's session and data. Only install plugins from trusted sources.
