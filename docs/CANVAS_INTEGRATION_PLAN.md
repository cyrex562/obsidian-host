# Canvas-Note Integration Plan

## Overview
This document outlines how the Canvas system integrates with the core note-taking features of Obsidian Host, ensuring a seamless experience between text-based and visual thinking.

## 1. Context Menu Integration

### "Add to Canvas" Action
Add a new option to the file explorer context menu for `.md` files.

**UI Flow**:
1. User right-clicks a note in File Explorer.
2. Selects "Add to Canvas...".
3. **Sub-menu** appears:
    *   *New Canvas*: Creates `Using [Note Name].canvas` and adds the note.
    *   *Existing Canvas*: Lists 5 most recent canvases + "Browse...".
4. On selection, opens the canvas and places the File Node at the center (or next empty space).

### "Create Canvas from Selection"
Enable bulk-add functionality.

**UI Flow**:
1. User creates a multi-selection in File Explorer (Ctrl/Shift + Click).
2. Right-click -> "Create Canvas from Selection".
3. Prompt for filename.
4. Create new canvas, arranging selected notes in a grid or circle layout.

## 2. Embedding Canvases in Notes

Allow canvases to be viewed directly within markdown documents, similar to image embeds.

### Syntax
Standard markdown embed syntax: `![[My Diagram.canvas]]`

### Rendering Logic
1.  **Parser**: Detect `.canvas` extension in wikilinks.
2.  **Container**: Create a `div.canvas-embed` container.
3.  **Viewport**:
    *   Read `metadata.viewport` from the canvas file to determine start view.
    *   Or, calculate bounding box of all nodes and "Zoom to Fit".
4.  **Interactivity**:
    *   *Read-only mode* by default (pan/zoom allowed, moving nodes disabled).
    *   "Open in new tab" button overlay.

## 3. Bi-directional Linking

Ensure the Graph View and Backlinks panel respect Canvas connections.

### Forward Links
*   If `MyCanvas.canvas` contains a File Node for `NoteA.md`, then `MyCanvas` links to `NoteA`.
*   **Graph View**: Show an arrow `MyCanvas -> NoteA`.

### Backlinks
*   If `NoteB.md` embeds `![[MyCanvas.canvas]]`, then `NoteB` links to `MyCanvas`.
*   **Backlinks Panel**: When viewing `NoteA.md`, show `MyCanvas.canvas` in the "Linked Mentions" section.

### Edge Semantics (Advanced)
If a canvas edge connects `NoteA` -> `NoteB` with a label "supports", technically the canvas is asserting a relationship.
*   *Future Feature*: Semantic link indexing where `NoteA` implicitly links to `NoteB` via the canvas context.

## 4. Canvas Thumbnail Previews

Generate static previews for canvas files to be used in:
*   File Explorer hover previews.
*   Quick Switcher.
*   "Recent Files" dashboard.

### Rendering Strategy
Since we cannot render the full WebGL/Canvas context in a tiny popup efficiently:
1.  **On Save**: Generate a low-res PNG or SVG snapshot of the canvas content.
2.  **Storage**: Store as a hidden attachment (e.g., `.obsidian/cache/thumbnails/canvas_id.png`) or embedded in the file metadata (if small).
3.  **Fallback**: If no thumbnail exists, render a generic "Board" icon with the number of nodes (e.g., "5 Nodes, 3 Edges").

## 5. Canvas Templates

Extend the Templates System (Task 16.4) to support `.canvas` files.

### Template Storage
Store canvas templates in the `Templates/` folder alongside markdown templates.

### Variable Substitution
Apply variable replacement to Text Nodes within the canvas.
*   `{{date}}` in a text node becomes `2024-01-24`.
*   `{{title}}` becomes the new canvas filename.

### Use Cases
*   **Project Dashboard**: A pre-arranged grid of Group Nodes ("To Do", "Doing", "Done").
*   **Brainstorming**: A central Topic node with 4 radiating branches.
*   **Flowchart**: Standard Start/End nodes connected by arrows.

## 6. Integration Test Scenarios

### Workflow 1: The Research Board
1.  User reads `Article.md`.
2.  Right-click "Add to new Canvas".
3.  Canvas opens. User drags `Reference.md` from sidebar onto canvas.
4.  User draws connection between `Article` and `Reference`.
5.  User saves. Checks Graph View -> verifies connections exist.

### Workflow 2: The Dashboard Embed
1.  User opens `Weekly Update.md`.
2.  Types `![[Project Status.canvas]]`.
3.  Preview mode shows the live canvas diagram.
4.  User can pan/zoom the diagram inside the note.

## API Extensions

### CanvasService extensions
```typescript
interface CanvasIntegration {
    // Add a file to a canvas on disk
    addToCanvas(canvasPath: string, notePath: string): Promise<void>;

    // Generate a new canvas file from a list of notes
    createFromNotes(notePaths: string[], title: string): Promise<string>;
    
    // Get all files referenced by a canvas
    getReferencedFiles(canvasPath: string): Promise<string[]>;
}
```

This plan ensures Canvas is not just a standalone tool but a deeply integrated part of the Obsidian Host ecosystem.
