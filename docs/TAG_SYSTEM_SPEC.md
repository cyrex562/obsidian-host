# Tag System Specification

## Overview
This document specifies the behavior of the Tag System in Obsidian Host, including parsing, nested tags, the Tag Explorer UI, and search integration.

## 1. Tag Parsing

### Sources
Tags are derived from two sources:
1.  **Inline Content**: `#tag` tokens found in the Markdown body.
    *   Regex: `/(^|\s)#[a-zA-Z0-9_\-\/]+(\s|$)/`
    *   Exclusions: Should ignore `#` in code blocks, math blocks, and URLs.
2.  **Frontmatter**: The `tags` key (list or comma-separated string).

### Normalization
*   Case-insensitive for search, but case-preserving for display?
    *   *Decision*: **Case-Insensitive**. `#Idea` and `#idea` are the same tag. Lowercase is canonical.
*   Invalid characters: Tags cannot contain spaces, commas, or `#` (except leading).
*   Nested delimiter: `/` (forward slash).

## 2. Nested Tags (Hierarchy)

Support for hierarchical organization using `/`.

*   **Raw Tag**: `#project/ui/login`
*   **Implied Hierarchy**:
    *   `project`
        *   `ui`
            *   `login`

### Query Logic
Searching for `#project` should optionally return all sub-tags (`#project/ui`, `#project/backend`).

## 3. Tag Explorer UI (Sidebar View)

A tree-view component visualizing the tag hierarchy.

### Structure
```
Tags
├── #project (5)
│   ├── #project/ui (2)
│   │   └── #project/ui/login (1)
│   └── #project/api (3)
├── #personal (10)
└── #waiting-for (2)
```

### Interactions
*   **Click**: Performs a global search filter `tag:#clicked_tag`.
*   **Drag & Drop**:
    *   Drag a file *onto* a tag -> Adds that tag to the file's frontmatter.
    *   Drag a tag *onto* another tag -> Bulk rename (e.g., move `#idea` to `#wip/idea`). (Advanced feature).
*   **Context Menu**:
    *   "Rename Tag" -> Updates all occurrences in vault.
    *   "Delete Tag" -> Removes string from all files.

## 4. Auto-Complete (Suggester)

When typing `#` in the editor:

1.  Trigger `TagSuggester` after `#`.
2.  Query `FileService` (or `TagIndex`) for all known tags.
3.  Filter list by typed characters (fuzzy match).
4.  Display list sorted by usage count (frequency).
5.  On Enter -> Insert tag text.

## 5. Search Integration

Extend the Search Service (Feature 9/19) to support structured tag queries.

*   `tag:work` -> Matches `#work`.
*   `tag:#work` -> Matches `#work`.
*   `tag:project` -> Matches `#project` AND `#project/ui` (prefix match).
*   `-tag:done` -> Exclude notes with this tag.

## 6. Performance Strategy

*   **Tag Index**:
    *   Maintain an inverted index `Map<TagName, Set<FileId>>`.
    *   Also maintain `Map<ParentTag, Set<ChildTag>>` for fast tree rendering.
*   **Updates**:
    *   On file save, re-scan regex and update index.
    *   Delta update: Remove old tags for this file, add new tags.

This ensures tags are a first-class citizen for organization, scaling up to thousands of tags efficiently.
