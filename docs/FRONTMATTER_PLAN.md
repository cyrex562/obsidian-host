# Frontmatter Management Plan

## Overview
This document outlines the UI and logic for managing YAML frontmatter (metadata) in Obsidian Host notes, equivalent to Obsidian's "Properties" view.

## 1. Frontmatter Editor UI ("Properties View")

### Interaction Model
A specialized UI component rendered at the top of the note (or in a separate sidebar panel) that provides a form-based interface for editing YAML.

```html
<div class="frontmatter-editor">
    <div class="property-row">
        <input class="property-key" value="tags">
        <div class="property-value-container">
            <span class="tag-pill">#work</span>
            <span class="tag-pill">#important</span>
            <button class="add-tag-btn">+</button>
        </div>
    </div>
    <div class="property-row">
        <input class="property-key" value="status">
        <select class="property-value">
            <option value="draft">Draft</option>
            <option value="published">Published</option>
        </select>
    </div>
</div>
```

### Modes
1.  **Read Mode**: Rendered table showing key-value pairs nicely formatted. Dates are localized. Links are clickable.
2.  **Edit Mode**: Input fields. Changes serialize back to YAML and update the file text immediately (or on blur).
3.  **Source Mode**: Direct text editing of the YAML block at the top of the file (standard markdown editor).

## 2. Property Types (Task 18.2 Preview)

The editor detects the type of value and renders the appropriate widget:
*   **Text**: Standard text input.
*   **Tags**: Pill inputs with auto-complete from existing tags.
*   **List**: Dynamic array builder.
*   **Checkbox**: Toggle switch.
*   **Date**: Calendar picker.

## 3. Frontmatter Templates

Integration with the **Templates System (Task 16.4)**.

### Concept
Allow standard sets of properties to be applied to notes.

### Implementation
1.  **Template Definitions**: Define property schemas in JSON or special Markdown files.
    ```json
    // .obsidian/templates/properties/meeting.json
    {
        "type": "meeting",
        "properties": {
            "attendees": ["list"],
            "date": "date",
            "project": "link"
        }
    }
    ```
2.  **Apply Template**: User clicks "Add Property Group" -> Selects "Meeting" -> Editor pre-fills keys.
3.  **Inheritance**: (Advanced) define that all notes in `People/` folder automatically get `email` and `phone` properties.

## 4. Parser Logic

We currently use the `FrontmatterService` (Rust) for parsing.

### Updates Required
*   **Preservation**: Ensure comments in YAML are preserved (or explicitly document they are lost on edit).
*   **Ordering**: Maintain key order when updating a single value.

### Data Flow
1.  **Read**: `GET /api/notes/{id}` returns content.
2.  **Extract**: Frontend extracts YAML block.
3.  **Parse**: `js-yaml` parses to JSON object.
4.  **Edit**: User modifies JSON object via UI.
5.  **Serialize**: `js-yaml` dumps to string.
6.  **Write**: Frontend replaces top block of text and saves.

## 5. Validation

*   **Syntax**: Prevent saving if YAML is invalid (show error line).
*   **Type Safety**: If a key `age` is usually a number, warn if user enters text.

## 6. Integration Test Strategy

*   **Round-Trip**: Load file -> Edit property -> Save -> Load again -> Verify value matches.
*   **Complex Nested**: Verify lists of objects `[{name: 'a'}, {name: 'b'}]` are handled correctly.
*   **Empty**: Adding frontmatter to a file that had none.

This plan solidifies the Frontmatter feature set, ensuring we match the "Properties" capability of modern editors.
