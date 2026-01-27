# Canvas Data Model Specification

## Overview
The Canvas feature provides a visual, infinite canvas for organizing and connecting notes, similar to Obsidian's Canvas plugin. This document specifies the data model and file format.

## Canvas File Format

### File Extension
Canvas files use the `.canvas` extension and are stored as JSON.

**Example**: `MyCanvas.canvas`

### File Structure

```json
{
  "nodes": [
    {
      "id": "node-1",
      "type": "file",
      "file": "Notes/My Note.md",
      "x": 100,
      "y": 200,
      "width": 400,
      "height": 300,
      "color": "1"
    },
    {
      "id": "node-2",
      "type": "text",
      "text": "This is a text node",
      "x": 600,
      "y": 200,
      "width": 300,
      "height": 200,
      "color": "2"
    },
    {
      "id": "node-3",
      "type": "link",
      "url": "https://example.com",
      "x": 1000,
      "y": 200,
      "width": 400,
      "height": 300
    }
  ],
  "edges": [
    {
      "id": "edge-1",
      "fromNode": "node-1",
      "fromSide": "right",
      "toNode": "node-2",
      "toSide": "left",
      "color": "0",
      "label": "relates to"
    }
  ],
  "metadata": {
    "version": "1.0.0",
    "created": "2024-01-24T12:00:00Z",
    "modified": "2024-01-24T14:30:00Z",
    "viewport": {
      "x": 0,
      "y": 0,
      "zoom": 1.0
    }
  }
}
```

## Data Structures

### Canvas

```typescript
interface Canvas {
    nodes: CanvasNode[];
    edges: CanvasEdge[];
    metadata: CanvasMetadata;
}
```

### Node Types

#### Base Node

```typescript
interface CanvasNodeBase {
    id: string;              // Unique identifier (e.g., "node-1")
    type: NodeType;          // "file" | "text" | "link" | "group"
    x: number;               // X position in canvas coordinates
    y: number;               // Y position in canvas coordinates
    width: number;           // Node width in pixels
    height: number;          // Node height in pixels
    color?: string;          // Color identifier (0-6)
    zIndex?: number;         // Stacking order
}

type NodeType = "file" | "text" | "link" | "group";
```

#### File Node

Links to a note in the vault:

```typescript
interface FileNode extends CanvasNodeBase {
    type: "file";
    file: string;            // Path to file (e.g., "Notes/My Note.md")
    subpath?: string;        // Optional heading/block reference
}
```

**Example**:
```json
{
  "id": "node-1",
  "type": "file",
  "file": "Notes/Project Ideas.md",
  "subpath": "#Implementation",
  "x": 100,
  "y": 200,
  "width": 400,
  "height": 300,
  "color": "1"
}
```

#### Text Node

Contains inline text/markdown:

```typescript
interface TextNode extends CanvasNodeBase {
    type: "text";
    text: string;            // Markdown content
}
```

**Example**:
```json
{
  "id": "node-2",
  "type": "text",
  "text": "# Important Note\n\nThis is a text node with **markdown**.",
  "x": 600,
  "y": 200,
  "width": 300,
  "height": 200,
  "color": "2"
}
```

#### Link Node

Embeds external content:

```typescript
interface LinkNode extends CanvasNodeBase {
    type: "link";
    url: string;             // URL to embed
}
```

**Example**:
```json
{
  "id": "node-3",
  "type": "link",
  "url": "https://example.com",
  "x": 1000,
  "y": 200,
  "width": 400,
  "height": 300
}
```

#### Group Node

Groups other nodes:

```typescript
interface GroupNode extends CanvasNodeBase {
    type: "group";
    label?: string;          // Optional group label
    background?: string;     // Background color
    nodes: string[];         // IDs of contained nodes
}
```

**Example**:
```json
{
  "id": "group-1",
  "type": "group",
  "label": "Research Notes",
  "x": 50,
  "y": 150,
  "width": 900,
  "height": 500,
  "background": "#f0f0f0",
  "nodes": ["node-1", "node-2"]
}
```

### Edges (Connections)

```typescript
interface CanvasEdge {
    id: string;              // Unique identifier
    fromNode: string;        // Source node ID
    fromSide?: Side;         // Connection point on source
    fromEnd?: EndType;       // Arrow/decoration at source
    toNode: string;          // Target node ID
    toSide?: Side;           // Connection point on target
    toEnd?: EndType;         // Arrow/decoration at target
    color?: string;          // Color identifier (0-6)
    label?: string;          // Optional edge label
    style?: LineStyle;       // Line style
}

type Side = "top" | "right" | "bottom" | "left";
type EndType = "none" | "arrow";
type LineStyle = "solid" | "dashed" | "dotted";
```

**Example**:
```json
{
  "id": "edge-1",
  "fromNode": "node-1",
  "fromSide": "right",
  "fromEnd": "none",
  "toNode": "node-2",
  "toSide": "left",
  "toEnd": "arrow",
  "color": "0",
  "label": "leads to",
  "style": "solid"
}
```

### Metadata

```typescript
interface CanvasMetadata {
    version: string;         // Canvas format version
    created: string;         // ISO 8601 timestamp
    modified: string;        // ISO 8601 timestamp
    viewport?: Viewport;     // Last viewport state
    settings?: CanvasSettings;
}

interface Viewport {
    x: number;               // Viewport center X
    y: number;               // Viewport center Y
    zoom: number;            // Zoom level (1.0 = 100%)
}

interface CanvasSettings {
    snapToGrid?: boolean;    // Enable grid snapping
    gridSize?: number;       // Grid size in pixels
    showGrid?: boolean;      // Show background grid
    theme?: string;          // Canvas theme
}
```

**Example**:
```json
{
  "version": "1.0.0",
  "created": "2024-01-24T12:00:00Z",
  "modified": "2024-01-24T14:30:00Z",
  "viewport": {
    "x": 500,
    "y": 400,
    "zoom": 1.2
  },
  "settings": {
    "snapToGrid": true,
    "gridSize": 20,
    "showGrid": true
  }
}
```

## Color Palette

Canvas supports 7 predefined colors:

```typescript
const CANVAS_COLORS = {
  "0": "#808080",  // Gray (default)
  "1": "#ff6b6b",  // Red
  "2": "#4ecdc4",  // Teal
  "3": "#45b7d1",  // Blue
  "4": "#96ceb4",  // Green
  "5": "#ffeaa7",  // Yellow
  "6": "#dfe6e9",  // Light gray
};
```

## Serialization

### Saving Canvas

```typescript
class CanvasSerializer {
    serialize(canvas: Canvas): string {
        return JSON.stringify(canvas, null, 2);
    }

    async saveCanvas(vaultId: string, path: string, canvas: Canvas): Promise<void> {
        const json = this.serialize(canvas);
        await api.writeFile(vaultId, path, json);
    }
}
```

### Loading Canvas

```typescript
class CanvasDeserializer {
    deserialize(json: string): Canvas {
        const data = JSON.parse(json);
        this.validate(data);
        return data as Canvas;
    }

    async loadCanvas(vaultId: string, path: string): Promise<Canvas> {
        const content = await api.readFile(vaultId, path);
        return this.deserialize(content);
    }

    private validate(data: any): void {
        if (!data.nodes || !Array.isArray(data.nodes)) {
            throw new Error('Invalid canvas: missing nodes array');
        }
        if (!data.edges || !Array.isArray(data.edges)) {
            throw new Error('Invalid canvas: missing edges array');
        }
        if (!data.metadata || !data.metadata.version) {
            throw new Error('Invalid canvas: missing metadata');
        }

        // Validate each node
        for (const node of data.nodes) {
            this.validateNode(node);
        }

        // Validate each edge
        for (const edge of data.edges) {
            this.validateEdge(edge);
        }
    }

    private validateNode(node: any): void {
        if (!node.id || !node.type || node.x === undefined || node.y === undefined) {
            throw new Error(`Invalid node: ${JSON.stringify(node)}`);
        }

        switch (node.type) {
            case 'file':
                if (!node.file) throw new Error('File node missing file path');
                break;
            case 'text':
                if (!node.text) throw new Error('Text node missing text content');
                break;
            case 'link':
                if (!node.url) throw new Error('Link node missing URL');
                break;
            case 'group':
                if (!node.nodes || !Array.isArray(node.nodes)) {
                    throw new Error('Group node missing nodes array');
                }
                break;
            default:
                throw new Error(`Unknown node type: ${node.type}`);
        }
    }

    private validateEdge(edge: any): void {
        if (!edge.id || !edge.fromNode || !edge.toNode) {
            throw new Error(`Invalid edge: ${JSON.stringify(edge)}`);
        }
    }
}
```

## Node Operations

### Creating Nodes

```typescript
class CanvasNodeFactory {
    createFileNode(file: string, x: number, y: number): FileNode {
        return {
            id: this.generateId(),
            type: 'file',
            file,
            x,
            y,
            width: 400,
            height: 300,
        };
    }

    createTextNode(text: string, x: number, y: number): TextNode {
        return {
            id: this.generateId(),
            type: 'text',
            text,
            x,
            y,
            width: 300,
            height: 200,
        };
    }

    createLinkNode(url: string, x: number, y: number): LinkNode {
        return {
            id: this.generateId(),
            type: 'link',
            url,
            x,
            y,
            width: 400,
            height: 300,
        };
    }

    createGroupNode(label: string, x: number, y: number, nodes: string[]): GroupNode {
        return {
            id: this.generateId(),
            type: 'group',
            label,
            x,
            y,
            width: 500,
            height: 400,
            nodes,
        };
    }

    private generateId(): string {
        return `node-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    }
}
```

### Creating Edges

```typescript
class CanvasEdgeFactory {
    createEdge(
        fromNode: string,
        toNode: string,
        options?: Partial<CanvasEdge>
    ): CanvasEdge {
        return {
            id: this.generateId(),
            fromNode,
            toNode,
            fromSide: 'right',
            toSide: 'left',
            toEnd: 'arrow',
            color: '0',
            style: 'solid',
            ...options,
        };
    }

    private generateId(): string {
        return `edge-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    }
}
```

## Canvas Manager

```typescript
class CanvasManager {
    private canvas: Canvas;
    private serializer: CanvasSerializer;
    private deserializer: CanvasDeserializer;

    constructor() {
        this.canvas = this.createEmptyCanvas();
        this.serializer = new CanvasSerializer();
        this.deserializer = new CanvasDeserializer();
    }

    createEmptyCanvas(): Canvas {
        return {
            nodes: [],
            edges: [],
            metadata: {
                version: '1.0.0',
                created: new Date().toISOString(),
                modified: new Date().toISOString(),
            },
        };
    }

    addNode(node: CanvasNode): void {
        this.canvas.nodes.push(node);
        this.updateModified();
    }

    removeNode(nodeId: string): void {
        this.canvas.nodes = this.canvas.nodes.filter(n => n.id !== nodeId);
        this.canvas.edges = this.canvas.edges.filter(
            e => e.fromNode !== nodeId && e.toNode !== nodeId
        );
        this.updateModified();
    }

    updateNode(nodeId: string, updates: Partial<CanvasNode>): void {
        const node = this.canvas.nodes.find(n => n.id === nodeId);
        if (node) {
            Object.assign(node, updates);
            this.updateModified();
        }
    }

    addEdge(edge: CanvasEdge): void {
        this.canvas.edges.push(edge);
        this.updateModified();
    }

    removeEdge(edgeId: string): void {
        this.canvas.edges = this.canvas.edges.filter(e => e.id !== edgeId);
        this.updateModified();
    }

    getNode(nodeId: string): CanvasNode | undefined {
        return this.canvas.nodes.find(n => n.id === nodeId);
    }

    getEdge(edgeId: string): CanvasEdge | undefined {
        return this.canvas.edges.find(e => e.id === edgeId);
    }

    getConnectedNodes(nodeId: string): CanvasNode[] {
        const connectedIds = new Set<string>();
        
        for (const edge of this.canvas.edges) {
            if (edge.fromNode === nodeId) {
                connectedIds.add(edge.toNode);
            }
            if (edge.toNode === nodeId) {
                connectedIds.add(edge.fromNode);
            }
        }

        return this.canvas.nodes.filter(n => connectedIds.has(n.id));
    }

    private updateModified(): void {
        this.canvas.metadata.modified = new Date().toISOString();
    }

    async save(vaultId: string, path: string): Promise<void> {
        await this.serializer.saveCanvas(vaultId, path, this.canvas);
    }

    async load(vaultId: string, path: string): Promise<void> {
        this.canvas = await this.deserializer.loadCanvas(vaultId, path);
    }
}
```

## Validation Rules

### Node Validation
- ✅ Unique IDs across all nodes
- ✅ Valid node type
- ✅ Required fields present
- ✅ File paths exist (for file nodes)
- ✅ Valid URLs (for link nodes)
- ✅ Positive dimensions

### Edge Validation
- ✅ Unique IDs across all edges
- ✅ Source and target nodes exist
- ✅ No self-loops (optional)
- ✅ Valid side values
- ✅ Valid end types

### Canvas Validation
- ✅ Valid version number
- ✅ Valid timestamps
- ✅ No orphaned edges
- ✅ No duplicate IDs

## Migration Strategy

### Version 1.0.0 → 1.1.0

```typescript
class CanvasMigrator {
    migrate(canvas: any, fromVersion: string, toVersion: string): Canvas {
        if (fromVersion === '1.0.0' && toVersion === '1.1.0') {
            return this.migrateV1ToV1_1(canvas);
        }
        throw new Error(`Unsupported migration: ${fromVersion} → ${toVersion}`);
    }

    private migrateV1ToV1_1(canvas: any): Canvas {
        // Add new fields, set defaults, etc.
        return {
            ...canvas,
            metadata: {
                ...canvas.metadata,
                version: '1.1.0',
            },
        };
    }
}
```

## Example Canvas

```json
{
  "nodes": [
    {
      "id": "node-1",
      "type": "file",
      "file": "Projects/Obsidian Host.md",
      "x": 0,
      "y": 0,
      "width": 400,
      "height": 300,
      "color": "3"
    },
    {
      "id": "node-2",
      "type": "text",
      "text": "# Key Features\n\n- Plugin system\n- Canvas view\n- Search",
      "x": 500,
      "y": 0,
      "width": 300,
      "height": 250,
      "color": "2"
    },
    {
      "id": "node-3",
      "type": "file",
      "file": "Tasks/Implementation.md",
      "x": 0,
      "y": 400,
      "width": 400,
      "height": 300,
      "color": "1"
    },
    {
      "id": "group-1",
      "type": "group",
      "label": "Project Overview",
      "x": -50,
      "y": -50,
      "width": 900,
      "height": 400,
      "nodes": ["node-1", "node-2"]
    }
  ],
  "edges": [
    {
      "id": "edge-1",
      "fromNode": "node-1",
      "fromSide": "bottom",
      "toNode": "node-3",
      "toSide": "top",
      "toEnd": "arrow",
      "label": "implements"
    },
    {
      "id": "edge-2",
      "fromNode": "node-1",
      "fromSide": "right",
      "toNode": "node-2",
      "toSide": "left",
      "toEnd": "arrow",
      "label": "describes"
    }
  ],
  "metadata": {
    "version": "1.0.0",
    "created": "2024-01-24T12:00:00Z",
    "modified": "2024-01-24T14:30:00Z",
    "viewport": {
      "x": 400,
      "y": 300,
      "zoom": 1.0
    },
    "settings": {
      "snapToGrid": true,
      "gridSize": 20,
      "showGrid": true
    }
  }
}
```

## Summary

✅ **File Format**: JSON-based .canvas files
✅ **Node Types**: File, Text, Link, Group
✅ **Edge System**: Connections with labels and styles
✅ **Serialization**: Save/load functionality
✅ **Validation**: Comprehensive validation rules
✅ **Metadata**: Version, timestamps, viewport
✅ **Color System**: 7 predefined colors
✅ **Migration**: Version migration strategy

The Canvas data model is fully specified and ready for implementation!
