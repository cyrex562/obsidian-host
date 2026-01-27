# Canvas Editing Implementation Plan

## Overview
This document details the interaction models and technical implementation strategies for editing operations within the Canvas view (Task 17.3). It builds upon the Data Model (17.1) and Rendering (17.2) specifications.

## 1. Interaction State Machine

The canvas relies on a state machine to handle complex user inputs (mouse/touch).

```typescript
type InteractionState = 
  | 'IDLE'              // Default state, waiting for input
  | 'PANNING'           // Moving the viewport (Middle mouse / Space + Left)
  | 'DRAGGING_NODE'     // Moving one or more nodes
  | 'RESIZING_NODE'     // Dragging a resize handle
  | 'DRAWING_EDGE'      // Creating a new connection
  | 'SELECTING_AREA'    // Marquee selection
  | 'EDITING_TEXT';     // Direct text input in a node

interface InteractionContext {
    startPos: Point;      // Screen coordinates where interaction started
    currentPos: Point;    // Current screen coordinates
    selection: Set<string>; // IDs of selected elements
    activeElementId: string | null; // ID of element being interacted with
    metaKey: boolean;     // Ctrl/Cmd status
    shiftKey: boolean;    // Shift status
}
```

## 2. Drag-and-Drop Nodes

### Interaction Model
1.  **Hit Test**: On `mousedown`, check if cursor is over a node.
2.  **Initiation**: If movement > 3px threshold, enter `DRAGGING_NODE` state.
3.  **Update**: On `mousemove`, update `x/y` of selected nodes. properties.
    *   Apply grid snapping if enabled (`Math.round(x / gridSize) * gridSize`).
    *   If dragging a **Group Node**, apply delta to all children.
4.  **Termination**: On `mouseup`, commit new positions to history stack.

### Collision & Snapping
*   **Grid Snapping**: visual feedback (ghost outline) when nearing grid lines.
*   **Alignment Guides**: Dynamic lines connecting centers/edges of other nodes when dragging.

## 3. Node Resizing

### Resize Handles
Each selected node renders 8 handles (corners + sides) when selected.

```typescript
enum ResizeHandle {
    TOP_LEFT, TOP, TOP_RIGHT,
    RIGHT, BOTTOM_RIGHT, BOTTOM,
    BOTTOM_LEFT, LEFT
}
```

### Logic
1.  **Hit Test**: Check against resize handle bounds (larger hit area than visual).
2.  **Constraint**: Calculate new `width/height`.
    *   Maintain Aspect Ratio if `Shift` is held.
    *   Minimum size constraints (e.g., 40x40px).
    *   Grid snapping for dimensions.
3.  **Anchor Point**: The opposite corner acts as the anchor (remains fixed).

## 4. Edge Drawing Tool

### Interaction
1.  **Anchor Points**: Nodes show "ports" (top/right/bottom/left) when hovered.
2.  **Drag Start**: Clicking a port starts `DRAWING_EDGE`.
3.  **Preview**: A bezier curve follows the mouse cursor.
4.  **Target Snapping**: When hovering another node/port, snap the end of the line to it.
5.  **Completion**: Release on a valid target -> Create Edge. Release elsewhere -> Cancel or Open Context Menu (to create new node).

### Path Finding
For complex layouts, edges shouldn't overlap nodes.
*   **Simple**: Cubic Bezier curves.
*   **Advanced**: A* or Orthogonal routing (future enhancement).

## 5. Text & Media Nodes

### Creation UX
*   **Double Click Canvas**: Creates a generic Text Node at position.
*   **Paste**:
    *   Text -> Create Text Node.
    *   Image/Media -> Upload file + Create Linked File Node.
    *   URL -> Create Link Node.
*   **Drag File from System/Sidebar**: Create File Node.

### Editing
*   **Text Nodes**: Switch to `contenteditable` div or embedded CodeMirror/Monaco instance on double-click. Auto-resize height based on content.
*   **Markdown Rendering**: Render raw markdown when editing, HTML preview when idle.

## 6. Node Grouping

### Group Container
*   **Creation**: Select multiple nodes -> Right Click -> "Group Selection".
*   **Behavior**: A Group Node acts as a visual container.
*   **Membership Logic**:
    *   Strict: Node must be fully inside to be grouped.
    *   Loose: Node center point determines grouping.
*   **Rendering**: Draw rectangular background behind contained nodes.

### Interactions
*   **Drag Group**: Moves all child nodes.
*   **Remove from Group**: Drag node outside group bounds.
*   **Add to Group**: Drag node inside group bounds + visual highlight.

## 7. Undo/Redo System

Implement the Command Pattern to manage history unique to the Canvas (separate from Vault history).

```typescript
interface Command {
    execute(): void;
    undo(): void;
}

class HistoryManager {
    private undoStack: Command[] = [];
    private redoStack: Command[] = [];

    push(cmd: Command) {
        this.undoStack.push(cmd);
        this.redoStack = []; // Clear redo on new action
        cmd.execute();
    }

    undo() {
        const cmd = this.undoStack.pop();
        if (cmd) {
            cmd.undo();
            this.redoStack.push(cmd);
        }
    }

    redo() {
        const cmd = this.redoStack.pop();
        if (cmd) {
            cmd.execute();
            this.undoStack.push(cmd);
        }
    }
}
```

### Serializable Actions
Every modification must map to a serializable payload:
*   `MOVE_NODES`: `{ ids: string[], dx: number, dy: number }`
*   `RESIZE_NODE`: `{ id: string, oldRect: Rect, newRect: Rect }`
*   `CREATE_EDGE`: `{ edge: CanvasEdge }`
*   `DELETE_ELEMENTS`: `{ nodes: CanvasNode[], edges: CanvasEdge[] }`

## 8. Test Plan (Workflow)

### User Stories
1.  **Brainstorming**: User creates 5 text nodes, rearranges them, and draws connections.
2.  **Reference Board**: User drags 3 images and 2 markdown notes onto canvas. Resizes images to be smaller. Groups them under "Resources".
3.  **Refactoring**: User selects a group of nodes, moves them to the right. User regrets the move, hits Undo. Everything snaps back.

### Edge Cases
*   Dragging a node that has 50 connected edges (Performance check).
*   Resizing a group smaller than its contained nodes (Constraint check).
*   Cyclic connections (Visual check).
*   Paste large image (Upload + Node creation flow).

## API Integration

### CanvasService Setup
The frontend `CanvasView` will communicate with `CanvasManager` (defined in 17.1).

```typescript
// Frontend -> Backend
onNodeMove(id: string, x: number, y: number) {
    this.canvasManager.updateNode(id, { x, y });
    this.requestSave(); // Debounced save
}
```

This plan provides the blueprint for implementing the interactive editing capabilities of the Canvas view.
