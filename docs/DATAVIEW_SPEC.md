# Dataview Implementation Specification

## Overview
This feature allows users to embed dynamic queries into their notes using code blocks. These blocks render as live tables, lists, or task collections based on the vault metadata.

## 1. Syntax (DQL)

We support a simplified SQL-like syntax inside a custom code block type.

### Code Block
```dataview
TABLE status, due_date
FROM "Projects"
WHERE status = "active"
SORT due_date ASC
```

### Commands

*   **View Types**:
    *   `TABLE [columns...]`: Renders a grid.
    *   `LIST [expression]`: Renders a bulleted list.
    *   `TASK [expression]`: Renders checkboxes (interactive).
    *   `CALENDAR [date_field]`: Renders items on a calendar view.

*   **Clauses**:
    *   `FROM`: Path, Folder, or Tag source.
    *   `WHERE`: Filter condition.
    *   `SORT`: Ordering field.
    *   `GROUP BY`: Group sorting.
    *   `LIMIT`: Max results.
    *   `FLATTEN`: Unroll lists.

## 2. Rendering Pipeline

### Detection
The Markdown Renderer (frontend) detects code blocks with language `dataview`.

### Execution
1.  **Parse**: Convert the DQL string into a structured query object (`QueryAST`).
2.  **Fetch**: Send query to the `SearchService` (or `MetadataService` defined in Task 18.3).
3.  **Render**:
    *   Swap the `<pre>` block with a reactive container (e.g., `<div class="dataview-container">`).
    *   Render the results using a Virtualized List/Table component.

## 3. Inline Queries

Allow small values to be embedded in text flow.

**Syntax**: `` `={ count(tag:#work) }` ``

**Use Case**:
*   "You have `={ count(tag:#todo) }` remaining tasks."
*   "Project Status: `={ this.status }`"

## 4. Aggregation Functions

Support basic data analysis:
*   `length(list)` / `count()`
*   `sum(field)`
*   `average(field)`
*   `min(field)` / `max(field)`
*   `link(path, display)`

## 5. Caching & Performance

Dynamic queries can be expensive.

1.  **Dependency Tracking**:
    *   A query `FROM "Projects"` depends on files in that folder.
    *   The renderer registers a listener for file events in that scope.
2.  **Invalidation**:
    *   On file change: Determine which active queries are affected.
    *   Re-run only affected queries.
3.  **Render Cache**:
    *   Keep the DOM output stable; only patch diffs.

## 6. JavaScript API (Advanced)

For power users, expose a JS execution environment.

```dataviewjs
const pages = dv.pages('"Projects"').where(p => p.status == 'active');
dv.header(2, "Active Projects");
dv.list(pages.map(p => p.file.link));
```

*   **Security**: This must be sandboxed or strictly opt-in via settings (Capability: `run_scripts`).

This specification completes the vision for turning the vault into a programmable database.
