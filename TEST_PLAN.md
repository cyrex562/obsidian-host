# Test Plan for Obsidian Host Editor Modes

This document outlines the manual verification steps to ensure all editor modes function correctly.

## 1. Raw Mode (Default)
**Objective**: Verify basic markdown editing and auto-save.

1.  Open a Markdown file (`.md`).
2.  Ensure "Raw" button is active in the toolbar.
3.  Type some text in the textarea.
4.  Verify the "dot" indicator appears on the tab (indicating dirty state).
5.  Wait 5 seconds.
6.  Verify the "dot" disappears (indicating auto-save completed).
7.  Reload the page. Verify the changes persisted.

## 2. Side-by-Side Mode
**Objective**: Verify real-time backend markdown rendering.

1.  Open a Markdown file.
2.  Click "Side by Side" mode.
3.  Verify the view splits into two panes: Editor (Left) and Preview (Right).
4.  Type markdown syntax (e.g., `# Heading`, `**Bold**`, `[[WikiLink]]`) in the left pane.
5.  Verify the right pane updates to show the rendered HTML.
    *   **Note**: The preview updates asynchronously via the backend API.
6.  Verify `[[WikiLink]]` renders as a clickable link (or anchor).

## 3. Formatted Mode (Syntax Highlighting)
**Objective**: Verify CodeJar integration and syntax highlighting.

1.  Open a Markdown file.
2.  Click "Formatted" mode.
3.  Verify the editor looks like a code editor (monospaced font).
4.  Verify syntax highlighting:
    *   Headers (`#`) should be larger/colored.
    *   Bold/Italic should be styled.
5.  Edit text. Verify editing feels smooth and auto-save works (wait 5s).

## 4. Rendered Mode (WYSIWYG)
**Objective**: Verify rich text editing and round-trip conversion.

1.  Open a Markdown file with varied syntax (Headers, Lists, Links).
2.  Click "Rendered" mode.
3.  Verify the content displays as Rich Text (no markdown characters visible).
4.  **Editing**:
    *   Select text and use the toolbar to Bold/Italicize.
    *   Create a list (Bulleted/Ordered).
5.  **Round-Trip Verification**:
    *   Switch back to "Raw" mode.
    *   Verify the HTML changes were correctly converted back to Markdown (e.g., `<b>` -> `**`, `<li>` -> `- `).
    *   Note: Some formatting (like extra whitespace) might be normalized by the conversion engine (Turndown).

## 5. Persistence
**Objective**: Verify user preference is saved.

1.  Select "Side by Side" mode.
2.  Reload the browser page (F5).
3.  Verify the editor opens in "Side by Side" mode automatically.
4.  Repeat for "Rendered" mode.
