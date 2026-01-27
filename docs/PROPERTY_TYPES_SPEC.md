# Property Types Specification

## Overview
This document defines the behavior, validation, and serialization logic for each supported Frontmatter property type in Obsidian Host.

## Base Architecture

All properties implement this interface:

```typescript
interface PropertyHandler<T> {
    type: string;
    validate(value: any): boolean;
    serialize(value: T): string | number | boolean | object;
    parse(raw: any): T;
    renderEditor(value: T, onChange: (val: T) => void): HTMLElement;
    renderView(value: T): HTMLElement;
}
```

## 1. Text Property (`text`)

*   **Logic**: Standard string storage.
*   **YAML**: `key: "value"`
*   **Editor**: `<input type="text">` or `<textarea>` for multiline.
*   **Validation**: Max length check (optional).
*   **Features**:
    *   URL auto-linking detection.
    *   Line break handling (`|` style in YAML).

## 2. Number Property (`number`)

*   **Logic**: Integers or Float values.
*   **YAML**: `cost: 10.5`
*   **Editor**: `<input type="number">`
*   **Validation**: `isNaN()` check.
*   **Formatting**:
    *   Decimal precision config.
    *   Currency symbols (optional view config).

## 3. Date & DateTime (`date`, `datetime`)

*   **Logic**: Temporal values.
*   **YAML**:
    *   Date: `modified: 2024-01-24`
    *   DateTime: `created: 2024-01-24T14:30:00Z`
*   **Editor**: `<input type="date">` and `<input type="datetime-local">`.
*   **Parsing**: Strict ISO-8601 preference. Graceful fallback for `YYYY-MM-DD`.
*   **View**: Localized string (`Jan 24, 2024`). Relative time option (`2 hours ago`).

## 4. Checkbox (`checkbox`)

*   **Logic**: Boolean flags.
*   **YAML**: `published: true`
*   **Editor**: Toggle switch or Checkbox input.
*   **View**: `Yes/No` badge or Check icon.

## 5. List/Tag (`list`, `tag`)

*   **Logic**: Arrays of strings.
*   **YAML**:
    ```yaml
    tags:
      - research
      - ideas
    ```
    or flow style: `tags: [research, ideas]`
*   **Editor**:
    *   "Tag Input": Typing text + Enter creates a pill.
    *   Autocomplete: Suggests tags existing in the implementation of `getAllCollatedTags()`.
*   **Validation**: No duplicate values.

## 6. Link (`link`)

*   **Logic**: Reference to another note.
*   **YAML**: `related: [[My Other Note]]`
*   **Editor**: Fuzzy search input (linking to Quick Switcher logic).
*   **Serialization**: Must wrap in `[[...]]`.
    *   *Alternative*: `related: "My Other Note.md"` (File path), depending on user config. Warning: Path links break if files move. Wiki-links are preferred.
*   **View**: Clickable internal link. Hover preview support.

## 7. Aliases (`aliases`)

*   **Special Type**: Used by the system for fuzzy finding.
*   **Behavior**: Always a List of Strings.
*   **Editor**: Same as Tags but strictly for alternative titles.

## 8. Migration & Type Safety

What happens if a property changes type? e.g., `priority: "High"` (Text) becomes `priority: 1` (Number).

1.  **Strict Mode**: The editor for "Priority" (configured as Number) will flag "High" as an invalid value.
2.  **Loose Mode**: The editor detects the value is a string and renders a Text input, with a warning "Expected Number".
3.  **Conversion**: "Convert to Number" button -> Attempts `parseInt/Float`.

## Implementation Strategy

We will build a **PropertyRegistry** mapping keys to types:

```typescript
const vaultConfig = {
    properties: {
        "status": "text", // or "select"
        "due_date": "date",
        "attendees": "list"
    }
};

function getPropertyType(key: string, value: any): string {
    // 1. Check explicit config
    if (vaultConfig.properties[key]) return vaultConfig.properties[key];
    
    // 2. Infer from value
    if (value instanceof Date) return "datetime";
    if (typeof value === "boolean") return "checkbox";
    if (Array.isArray(value)) return "list";
    // ...
    return "text";
}
```

This specification ensures robust handling of all metadata types required for a rich frontmatter management experience.
