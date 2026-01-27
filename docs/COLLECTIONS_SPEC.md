# Note Collections Specification

## Overview
Collections allow users to organize notes into arbitrary groups without changing the file system folder structure. Think of them as "Playlists" for your notes.

## 1. Data Model

Collections are stored in a configuration file (e.g., `.obsidian/collections.json`).

### Structure

```json
{
  "collections": [
    {
      "id": "col-123",
      "name": "Reading List 2024",
      "type": "manual",
      "items": [
        "Notes/Book A.md",
        "Notes/Article B.md"
      ],
      "parentId": null
    },
    {
      "id": "col-456",
      "name": "Project X Resources",
      "type": "smart",
      "query": "tag:#project-x AND type:image",
      "parentId": "col-123" // Nested collection
    }
  ]
}
```

## 2. Manual Collections

### Behavior
*   **Ordering**: Notes have a specific manual order (0, 1, 2...).
*   **Arbitrary**: Can contain notes from different folders.
*   **Non-Exclusive**: A note can belong to multiple collections.

### UI Integration - "Collections" Panel
*   **Drag-and-Drop**:
    *   Drag a note from File Explorer -> Drop onto Collection Name.
    *   Drag notes *within* the collection view to reorder them.
*   **Context Menu**: "Add to Collection..." -> Select existing collection.

## 3. Smart Collections (Reference to Task 19.2)
"Smart Collections" are effectively Saved Searches rendered within the Collections UI hierarchy. They update dynamically and cannot be manually reordered (sorted by query rules).

## 4. Collection Views

When a user clicks a collection, the main view area displays the contents.

### View Modes
1.  **List View**: Simple table (Name, Date, Path).
2.  **Grid View (Cards)**: Preview cards with thumbnail/snippet. Useful for "Mood Boards".
3.  **Book View**: Render notes sequentially as one long document (concept-linking).

## 5. Nesting & Hierarchy

Collections can be nested to create organizational trees independent of the filesystem.

*   `Research` (Folder)
    *   `Biology` (Collection)
        *   `Genetics` (Sub-Collection)

### Implementation
*   **UI**: Sidebar tree component similar to File Explorer but for Metadata-only objects.
*   **Logic**: Moving a parent collection moves the subtree.

## 6. Use Cases
*   **Curriculums**: "Learn Rust" collection (Sequence of 10 notes).
*   **Zines/Books**: Organizing chapters (Chapter 1, Chapter 2) where file sorting implies alphabetical order, but collections allow narrative order.
*   **Daily Queues**: "To Read Today".

## 7. Export Strategy

Users can export a collection to share it or consume it in other formats.

### Export Formats

#### 1. Merged Markdown (Book Mode)
Concatenates all notes in the collection into a single `.md` file.
*   **Structure**:
    ```markdown
    # Collection Name
    
    # Note 1 Title
    [Content of Note 1]
    
    ---
    
    # Note 2 Title
    [Content of Note 2]
    ```
*   **Reference Handling**:
    *   **Internal Links**: If Note 1 links to Note 2 (`[[Note 2]]`), rewrite link to anchor `#Note 2 Title`.
    *   **External Links**: Preserve as is.
    *   **Attachments**: Update image paths to be relative to the new single file location or embed as Base64 (optional).

#### 2. PDF Export
Generates a printable document of the collection.
*   **Engine**: `window.print()` or server-side `wkhtmltopdf`/`puppeteer`.
*   **Formatting**: Apply a specific print stylesheet (page breaks between notes).
*   **TOC**: Optionally generate a Table of Contents at the beginning based on Note Titles.

#### 3. ZIP Archive
Packages the collection as a discrete unit (portable folder).
*   **Contents**:
    *   `Collection Name/` (Folder)
        *   `Note A.md`
        *   `Note B.md`
        *   `Attachments/` (Only images used by these notes)
        *   `index.md` (Table of contents linking to the files)
*   **Use Case**: Sharing a "Project" with a colleague without sending the entire Vault.

### Implementation Checklist
- [ ] **Dependency Graph**: Calculate full closure of files (notes + embedded images).
- [ ] **Path Rewriting**: Adjust wikilinks to work in the new standalone context.
- [ ] **UI**: "Export Collection" modal with Format selection.

This feature decouples organization from storage location, adhering to the Zettelkasten principle of structural freedom.
