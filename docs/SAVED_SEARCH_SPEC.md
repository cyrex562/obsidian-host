# Saved Search & History Specification

## Overview
This feature allows users to persist complex search queries ("Smart Folders") and quickly access recent searches.

## 1. Saved Search (Smart Collections)

### Data Model
Saved searches are stored in the user's config directory (e.g., `.obsidian/saved_searches.json`).

```json
{
  "searches": [
    {
      "id": "search-123",
      "name": "Urgent Work",
      "query": "tag:#work AND status:urgent",
      "sort": "modified_desc",
      "created": "2024-01-24T12:00:00Z"
    },
    {
      "id": "search-456",
      "name": "Daily Logs",
      "query": "path:\"Daily Notes/2024\"",
      "sort": "created_asc"
    }
  ]
}
```

### UI Integration

1.  **Search Panel**:
    *   "Save current search" button next to query input.
    *   "Saved Searches" accordion section below the input.
2.  **Sidebar (File Explorer)**:
    *   Optional: Render saved searches as virtual folders at the top of the file list.
    *   Icon: Magnifying glass with a star.

### Interaction
*   **Click**: Executes the query and populates the search results view.
*   **Context Menu**: Rename, Delete, Edit Query.
*   **Drag-and-Drop**: Reorder saved searches in the sidebar.

## 2. Search History

### Logic
*   **Trigger**: Any successful search execution (after Enter key or 2s debounce).
*   **Storage**: Browser `localStorage` or session config.
*   **Limit**: Keep last 20 unique queries (LRU - Least Recently Used eviction).

### UI Integration
*   **Dropdown**: When focusing the search input, show a list of recent queries.
*   **Navigation**: Up/Down arrow keys in empty search bar cycle through history.

## 3. Dynamic "Smart Folders"

Saved searches can essentially act as dynamic folders.

### Implementation
*   In the File Explorer, a "Smart Folder" runs its query when expanded.
*   The "children" of this folder are the search results (virtual nodes).
*   **Limitations**: Read-only structure. You cannot drag a file *into* a smart folder unless we implement logic to auto-tag the file to match the query (Advanced).

## 4. Search Sharing (Deep Linking)

Enable sharing a search context via URL.

*   **Format**: `obsidian://search?query=tag%3Awork` or `https://host/app?search=tag%3Awork`.
*   **Action**: Opening this link launches the app and immediately executes the search.

## 5. Notifications (Alerts)

Allow users to be notified when *new* results match a saved search.

### Use Case
*   "Notify me if a new note is created with tag `#urgent`".

### Implementation
1.  **Daemon**: On file save, the `SearchService` checks the file against a list of "Watched Searches".
2.  **Check**: Does the file match the query?
    *   *Optimization*: Only regex/tag checks are cheap. Full text search might be expensive on every save.
3.  **Alert**: Trigger a UI toast or system notification.

## 6. Shortcuts

*   Register commands in the Command Palette:
    *   "Search: Run 'Urgent Work'"
    *   "Search: Run 'Daily Logs'"
*   Users can then bind hotkeys (e.g., `Ctrl+Alt+1`) to these specific searches.

This ensures that complex knowledge retrieval workflows can be standardized and repeated instantly.
