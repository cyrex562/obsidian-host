# Obsidian Host - User Guide

Welcome to Obsidian Host! This application allows you to host and edit your Obsidian vaults via a web interface.

## Getting Started

1.  **Launch the Application**: Run the `obsidian-host` binary or start the Docker container.
2.  **Access the Web UI**: Open your browser and navigate to `http://localhost:8080` (or your configured port).
3.  **Register a Vault**:
    -   Click "Manage Vaults" or the vault switcher in the sidebar.
    -   Enter a name for your vault.
    -   Enter the absolute path to your local Obsidian vault (folder containing your `.md` files).
    -   Click "Add Vault".
4.  **Open the Vault**: Select your vault from the list.

## Interface Overview

-   **Sidebar (Left)**: Displays the file tree of your current vault. Resizable and collapsible.
-   **Main Area**: Your updated editor/preview area. Supports tabs for multiple files.
-   **Tab Bar**: Shows open files.
-   **Tools (Top Right)**:
    -   **Search**: Full-text search across the vault.
    -   **Settings**: Theme and editor preferences.
    -   **Vault Switcher**: Switch between registered vaults.

## Working with Files

### Navigation
-   Click files in the sidebar to open them.
-   Click folders to expand/collapse them.
-   Use the **Quick Switcher** (`Ctrl+O` or `Cmd+O`) to jump to files by name.

### Editing
-   **Modes**:
    -   **Side-by-Side**: Editor on the left, live preview on the right.
    -   **Raw**: Plain markdown editor.
    -   **Preview**: Rendered view only.
    -   **Live Preview**: (Upcoming) WYSIWYG-like editing.
-   **Auto-Save**: Changes are automatically saved as you type.
-   **Wiki Links**: Type `[[` to trigger autocomplete for internal links.
-   **Images**: Drag and drop images into the editor to upload and embed them.

### File Operations
-   **Create**: Right-click a folder and select "New File/Folder".
-   **Rename/Move**: Right-click a file and select "Rename".
-   **Delete**: Right-click a file and select "Delete" (moves to `.trash` if configured).
-   **Upload**: Drag files into the sidebar or use the upload button.

### Supported File Types
-   **Markdown**: `.md` (Full editing support)
-   **Images**: `.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`, `.webp` (Viewer with zoom/pan)
-   **PDF**: `.pdf` (Native viewer with search and metadata)
-   **Audio**: `.mp3`, `.wav`, `.ogg` (Playback)
-   **Video**: `.mp4`, `.webm` (Playback)
-   **Code**: Syntax highlighting for `.js`, `.ts`, `.py`, `.rs`, `.java`, `.c`, `.cpp`, `.css`, `.html`, `.xml`, `.json`, `.yaml`, `.sh`, and more.
-   **Other**: Download option available for unsupported file types.

## Search
-   Click the Search icon or press `Ctrl+F` (in some contexts).
-   Type to search for content or filenames.
-   Results show context snippets with highlighted matches.
-   Click a result to open the file at the matching line.

## Settings
-   **Theme**: Toggle between Light and Dark mode.
-   **Editor**: Configure font size and default view mode.
-   **Window Layout**: Save your current split/tab layout.

## Multi-User Usage
Currently, the application is designed for single-user or trusted-network usage. Multiple users can access the interface simultaneously, and changes are synchronized in real-time via WebSockets. However, there is no user authentication or permission system yet.

## Troubleshooting

### Vault not loading
-   Ensure the path is absolute and exists on the server.
-   Check server logs for permission errors.

### Changes not syncing
-   Check browser console for WebSocket connection errors.
-   Ensure no firewall is blocking the WebSocket port (same as HTTP port).
-   If running in Docker, ensure volumes are mounted correctly.

### Images not showing
-   Ensure images are within the vault hierarchy.
-   Check if the image path contains special characters not yet handled.
