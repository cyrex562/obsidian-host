# Graph View Implementation Plan

## Overview
The Graph View visualizes the relationships between notes in the vault. This document outlines the architecture, rendering strategy, and interaction model for the force-directed graph.

## 1. Architecture

### Data Processing Pipeline
1.  **Vault Indexing**: The `WikiLinkService` (already implemented) provides the source of truth for all links.
2.  **Graph Generation**: Convert the vault index into a Node/Link format suitable for visualization.
    *   **Nodes**: One for every markdown file.
        *   Properties: `id` (path), `label` (filename), `size` (based on backlinks/word count), `group` (folder/tag).
    *   **Links**: One for every wiki-link.
        *   Properties: `source`, `target`, `weight`.

### Force Simulation Engine
We will use `d3-force` or a WebGL-optimized alternative (like `ngraph`) for physics simulation.

**Forces**:
*   **Charge**: Repulsion between all nodes to prevent overlap.
*   **Link Distance**: Spring force pulling connected nodes together.
*   **Center**: Gravity force keeping the graph centered in the viewport.
*   **Collision**: Hard boundary check to ensure node readability.

## 2. Rendering Strategy

### WebGL vs. Canvas
For vaults with >1000 notes, HTML5 Canvas or SVG becomes sluggish.
*   **Decision**: Uses **HTML5 Canvas** for small/medium vaults (<2000 nodes) for text sharpness.
*   **Optimization**: Use **WebGL** (e.g., PIXI.js or Three.js) for large vaults if performance degrades.

### Visual Style
*   **Nodes**: Circles. Color coded by folder or tag. Size logarithmic to connection count.
*   **Edges**: Thin lines. Opacity fades with distance or unselected state. Arrows indicate direction.
*   **Text**: Labels render only on hover or for high-importance nodes (high centrality) to avoid clutter.

## 3. Interaction Model

### Navigation
*   **Pan**: Left-click drag on background.
*   **Zoom**: Scroll wheel / Pinch.
*   **Focus**: Double-click a node to center and zoom in.

### Selection & Highlighting
*   **Hover Node**:
    *   Highlight node and immediate neighbors.
    *   Dim all other nodes/edges.
    *   Show full filename label.
*   **Click Node**:
    *   Open note in a side panel or main editor (configurable).
    *   Keep highlighting active.

## 4. Filtering & Grouping UI

### Search Filter
*   Text input to filter nodes by name content.
*   Matches are highlighted; non-matches are dimmed or hidden.

### Grouping Controls
*   **Color By**: Dropdown [Folder, Tag, None].
*   **Filters**:
    *   Toggle "Orphans" (nodes with no links).
    *   Toggle "Attachments" (images/PDFs in graph).
    *   Toggle "Tags" (tags as nodes).

### Physics Controls
*   Sliders for: `Repulsion Strength`, `Link Distance`, `Gravity`.
*   "Freeze" button to stop simulation.

## 5. Implementation Roadmap

### Phase 1: Data & Basic Render
1.  Extend `search_service` to expose a bulk "get all links" endpoint.
2.  Frontend: Implement basic `d3-force` simulation.
3.  Render dots and lines on HTML5 Canvas.

### Phase 2: Interactivity
1.  Implement Pan/Zoom transform logic.
2.  Add hover hit-testing (quadtree for performance).
3.  Link click events to `openFile()`.

### Phase 3: Advanced Features
1.  Add color-coding logic based on file paths.
2.  Implement "Local Graph" (only show neighbors of current active note).
3.  Build the Settings Panel for physics tweaking.

## 6. Export Strategy

### Image Export
1.  **Canvas**: `canvas.toDataURL()` allows instant PNG download.
2.  **SVG**: For high-quality print, traverse the current graph state and generate an SVG string.

## Technical Stack Recommendation
*   **Simulation**: `d3-force` (Standard, robust ecosystem).
*   **Rendering**: HTML5 `<canvas>` API (Good balance of performance and text capability).
*   **State Management**: Custom `GraphController` class managing the simulation loop (`requestAnimationFrame`).

This plan provides the architectural blueprint for building the Graph View, ensuring scalability from small personal wikis to large knowledge bases.
