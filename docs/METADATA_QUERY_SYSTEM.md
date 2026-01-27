# Metadata Query & View System

## Overview
This system allows users to query, filter, and visualize metadata (frontmatter) across the entire vault. It functionality serves as the foundation for "Dataview"-like features and property-based navigation.

## 1. Query Engine

We need a way to efficiently query files based on their frontmatter.

### Indexing
The `SearchService` (Backend) will maintain a specialized index for properties.
*   **Structure**: `Map<PropertyKey, Map<PropertyValue, Set<FileId>>>`
*   **Example**: `tags -> { "project": {1, 2}, "urgent": {2, 5} }`

### Query Language (DQL - Data Query Language)
Simple SQL-like syntax for filtering.

*   `FROM "folder/path"`
*   `WHERE status = "active" AND priority > 1`
*   `SORT date DESC`
*   `LIMIT 20`

## 2. Global Property Views

### "All Properties" Dashboard
A system-level view listing all known property keys in the vault.
*   **Columns**: Key Name, Type (inferred), Usage Count.
*   **Action**: Click a key (e.g., "Review Status") to see all files containing it.

### Property Values View
Drilling down into a specific key (e.g., `author`).
*   **Visualization**: Bar chart of most common authors.
*   **List**: Table of unique values and their frequency.
*   **Bulk Rename**: Change `author: "J. Doe"` to `"John Doe"` across 50 files.

## 3. Auto-Complete System

When editing properties in a note, smart suggestions are critical.

### Data Source
The Aggregation Service scans the vault on startup (and incrementally updates on file save) to build a dictionary of known values for every key.

### Interaction
1.  User types key `sta...`.
2.  System suggests `status`, `start-date` (based on existing keys in vault).
3.  User selects `status` and types value `d...`.
4.  System suggests `draft`, `done` (values seen for `status` key in other files).

## 4. Property Statistics

For quantitative analysis of the vault.

*   **Completion Rate**: "70% of project notes have a `due_date`".
*   **Distribution**: "Most common tag is `#work` (150 notes)".
*   **Timeline**: Chart of `created` dates showing writing activity over time.

## 5. Implementation Architecture

### Backend (Rust)
*   `MetadataIndex`: In-memory struct optimized for property lookups.
*   `MetadataService`:
    *   `get_all_keys() -> Vec<String>`
    *   `get_values_for_key(key: String) -> Vec<String>`
    *   `search_by_property(query: PropertyQuery) -> Vec<FileMetadata>`

### Frontend (TypeScript)
*   `PropertySuggester`: UI component verifying inputs against the backend cache.
*   `DataTableView`: Reusable React/Web Component for rendering query results.
    *   Supports sorting/filtering columns.
    *   Inline editing of cells (bulk edit).

## 6. Performance Strategy
*   **Debounce Indexing**: Don't re-index immediately on every keystroke. Wait for file save + 500ms.
*   **Pagination**: The "All Properties" view should paginate results if a property exists on 10,000 files.

This system transforms the vault from a collection of text files into a structured database.
