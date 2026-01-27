# Advanced Search Syntax Specification

## Overview
This document defines the query language for the "Enhanced Search" feature, allowing users to combine text search with structural filters, metadata queries, and logical operators.

## 1. Boolean Operators

Support standard operators with grouping parentheses.
*   **AND** (Implicit): `apple banana` -> contains both "apple" AND "banana"
*   **OR**: `apple OR banana` -> contains either
*   **NOT**: `-banana` or `NOT banana` -> does not contain "banana"
*   **Grouping**: `(apple OR banana) -cherry`

## 2. Global Text Search
*   **Standard**: Matches filename OR content.
*   **Quoted**: `"exact phrase"` matches absolute sequence.
*   **Regex**: `/^Daily/` matches regex pattern. (Performance warning: executed post-filter or on constrained set).

## 3. Field Filters (Facets)

Limit scope to specific components of a note.

| Filter | Description | Example |
| :--- | :--- | :--- |
| `file:` / `path:` | Filename or folder path | `path: "Daily Notes"` |
| `content:` | Only body text (ignore title) | `content: "important"` |
| `tag:` | Hashtags (including nested) | `tag: #work` |
| `line:` | Matches if keywords appear together on line | `line:(task urgent)` |
| `section:` | Matches within a header section | `section:(# Goals)` |

## 4. Property Queries (Integration with Feature 18)

Directly query frontmatter fields using array/comparison syntax.

*   `[status: active]`
*   `[priority > 1]`
*   `[due-date < 2024-02-01]`

## 5. Date Search

Special semantics for date fields (`created`, `modified`, frontmatter dates).

*   **Relative**: `created: today`, `modified: -7days` (last 7 days).
*   **Absolute**: `date: 2024-01-24`.
*   **Range**: `date: 2024-01-01..2024-12-31`.

## 6. Query Builder UI

A visual component to construct these queries without typing syntax.

### Components
1.  **Scope Dropdown**: Path/Folder selector.
2.  **Filter Rows**:
    *   `[ AND/OR ] [ Field ] [ Operator ] [ Value ]`
    *   Example: `[ AND ] [ Tag ] [ is ] [ #work ]`
3.  **Result Preview**: Live update of matching file count.

## 7. Implementation Architecture

### Query Parser
1.  **Tokenize**: Split string into tokens, preserving quotes and parentheses.
2.  **AST Construction**: Build an Abstract Syntax Tree.
    ```json
    { "op": "AND", "left": { "type": "term", "val": "apple" }, "right": { "op": "NOT", ... } }
    ```
3.  **Executor**:
    *   **Backend (Rust)**: Use `tantivy` index for text fields. Use `MetadataIndex` for properties.
    *   **Optimization**: Execute restrictive filters (Path, Tag) first to reduce the set for full-text scanning.

### Search Highlights
The API should return:
*   File Metadata.
*   Context Snippets: " ... found text **keyword** here ... ".
*   Line numbers of matches.

This syntax aligns with industry standards (like GitHub or Gmail search), ensuring a minimal learning curve for users while offering deep power for power users.
