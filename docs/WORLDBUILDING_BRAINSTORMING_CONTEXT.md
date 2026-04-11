# Codex App and Worldbuilding Plugin Overview

This document is meant as a **brainstorming handoff** for discussing future worldbuilding features, especially new entity types, richer relationships, and workflow improvements.

## What Codex is

Codex is a self-hosted app for working with Obsidian-style vaults.

- **Backend:** Rust + Actix Web
- **Frontend:** Vue 3 + TypeScript + Vuetify
- **Storage model:** markdown files in vault folders, plus SQLite metadata/indexes
- **Live sync:** filesystem watcher + WebSocket updates

In practice, the app is a structured UI on top of ordinary markdown notes. A note can still be "just a note," but Codex can also recognize some notes as **typed entities** by reading structured frontmatter.

## Core app model

At a high level, the app works like this:

1. A vault is registered with the server.
2. Codex reads the vault file tree and serves it through the web UI.
3. Notes are opened in tabs and edited in different modes.
4. Background indexing extracts searchable content and entity metadata.
5. File changes are broadcast to clients in real time.

The important design idea for worldbuilding is that **entities are still markdown files**. The plugin does not introduce a separate database-first authoring model. Instead, it layers:

- entity schemas
- relation schemas
- templates
- UI affordances

on top of normal markdown notes.

## Editing modes that matter for worldbuilding

Worldbuilding content currently spans two layers:

1. **Structured frontmatter** for machine-readable data
2. **Markdown body/prose** for freeform writing

The current entity creation flow opens new entities in **structural editor mode**, which is important because the plugin is designed around structured fields plus prose, not prose alone.

## Plugin system in Codex

Codex has a plugin system, but the current worldbuilding plugin is mostly **schema-driven**, not a large custom app embedded in the frontend.

A plugin can contribute:

- a manifest
- entity type definitions
- relation type definitions
- template files
- optional frontend/runtime code

For the built-in worldbuilding plugin:

- the manifest lives at `plugins/worldbuilding/manifest.json`
- entity schemas live in `plugins/worldbuilding/entity_types/*.toml`
- relation schemas live in `plugins/worldbuilding/relation_types/*.toml`
- templates live in `plugins/worldbuilding/templates/*.md`
- `plugins/worldbuilding/main.js` is currently minimal

That means most current behavior comes from **schema + template files**, with the app's core frontend/backend doing the heavy lifting.

## How the worldbuilding plugin works

## 1. Registration and discovery

The backend loads plugin manifests and schema files from the bundled `plugins/` directory in the active release.

The worldbuilding manifest declares:

- plugin id: `com.codex.worldbuilding`
- human name: `Worldbuilding`
- built-in entity types
- built-in relation types
- built-in labels

This plugin currently defines four entity types:

| Entity type | Purpose | Labels | Template |
| --- | --- | --- | --- |
| Character | People / named individuals | `graphable`, `person` | `templates/character.md` |
| Faction | Groups / organizations | `graphable`, `organization` | `templates/faction.md` |
| Location | Places / areas | `graphable`, `place` | `templates/location.md` |
| Event | Historical or narrative events | `graphable`, `event` | `templates/event.md` |

It also defines four relation types:

| Relation | Meaning | Typical direction |
| --- | --- | --- |
| `member_of` | person -> organization | directed |
| `located_in` | thing -> place | directed |
| `participated_in` | thing -> event | directed |
| `knows` | person <-> person | undirected |

## 2. Entity schemas

Each entity type is defined in TOML.

A schema provides:

- display name
- icon/color
- template path
- labels
- `display_field`
- `show_on_create`
- field definitions

The most important fields in the schema design are:

- **`display_field`**: which field should act like the human title
- **`show_on_create`**: which fields appear in the quick-create dialog
- **field `type`**: string, text, enum, date, entity reference, etc.
- **`relation` on entity_ref fields**: tells Codex what relationship to create or interpret

This is why the plugin already feels semi-structured without needing custom code for every entity type.

## 3. Templates

When the user creates an entity, Codex asks the backend for the entity template:

- `GET /api/vaults/{vault_id}/entity-template?type={type_id}`
- `GET /api/plugins/entity-types/{type_id}/template`

The template is usually plugin-provided markdown with frontmatter, for example:

- `codex_type`
- `codex_plugin`
- `codex_labels`
- typed fields like `full_name`, `status`, `location`, etc.

If the template file is missing, the backend can generate a **minimal fallback template** from the entity schema.

## 4. Entity creation UX

There are now two main creation entry points for worldbuilding entities.

### A. Plugin manager quick actions

The Plugins modal shows worldbuilding shortcuts such as:

- New Character
- New Location
- New Faction
- New Event

These are powered by the registered entity types, filtered to the worldbuilding plugin.

### B. New Note dialog template handoff

The sidebar's **New note** dialog now supports a **Template** selector.

Behavior:

- if no entity templates are available, it behaves like a normal note dialog
- if entity types are available, the dialog shows:
  - `Regular note`
  - one option per entity type, such as `Character entity`
- choosing an entity template changes the action from **Create** to **Continue**
- clicking **Continue** opens the **New Entity** dialog with:
  - the entity type preselected
  - the filename prefilled

This is the current answer to the usability problem of "I should be able to create a character/place/etc. directly instead of editing frontmatter by hand."

## 5. New Entity dialog behavior

The `NewEntityDialog` is the main structured entity-creation UI.

It currently supports:

- selecting an entity type
- entering a filename
- optionally choosing a folder
- filling quick fields from `show_on_create`
- enum quick fields
- opening the created file immediately

Creation flow:

1. Load entity types from `GET /api/plugins/entity-types`
2. Pick a type
3. Fetch the type template
4. Patch frontmatter with:
   - `codex_type`
   - `codex_plugin`
   - `codex_labels`
   - display field/title
   - quick-create field values
5. Create the markdown file in the vault
6. Open it in structural editor mode

## 6. How Codex recognizes entities

Worldbuilding entities are fundamentally just markdown notes with specific frontmatter.

The important keys are:

- `codex_type`
- `codex_plugin`
- `codex_labels`

Example idea:

```yaml
---
codex_type: character
codex_plugin: com.codex.worldbuilding
codex_labels:
  - graphable
  - person
full_name: Lyra Voss
status: Active
faction: Skyguard
location: White Harbor
summary: Scout captain and smuggler-hunter.
---
```

The prose body remains ordinary markdown under the frontmatter.

## 7. How relationships work

Relations are schema-defined, not inferred from arbitrary text alone.

Examples from the built-in schemas:

- a character's `faction` field is an `entity_ref` that maps to `member_of`
- a character's `location` field maps to `located_in`
- an event's `location` field also maps to `located_in`
- a location's `parent_location` maps to `located_in`

So the system treats frontmatter references as structured worldbuilding links, which can later feed:

- entity pages
- relation panels
- graph views
- filtering/search

The graph endpoint is:

- `GET /api/vaults/{vault_id}/graph`

It returns nodes and edges built from entity and relation records for the vault.

## 8. Current built-in worldbuilding model

### Character

Quick-create fields:

- `full_name`
- `status`
- `faction`

Other fields:

- `location`
- `birth_date`
- `summary`

### Faction

Quick-create fields:

- `full_name`
- `type`

Other fields:

- `headquarters`
- `summary`

### Location

Quick-create fields:

- `full_name`
- `type`

Other fields:

- `parent_location`
- `summary`

### Event

Quick-create fields:

- `full_name`
- `date`
- `location`

Other fields:

- `summary`

## 9. Current strengths of the plugin

The current design already has a few strong properties:

- **markdown-native**: content still lives in normal files
- **schema-driven**: adding many behaviors does not require a ton of custom UI code
- **typed relations**: entity references can map to explicit relationship semantics
- **progressive enhancement**: users can still write plain notes without using the system
- **template-backed**: each entity starts with a predictable structure

## 10. Current limitations

These are the main practical limits of the current worldbuilding system:

### Limited type set

Only four built-in entity types ship today:

- character
- faction
- location
- event

### Flat creation model

The current creation flow is much better than before, but still fairly simple:

- one dialog
- a small set of quick fields
- no guided multi-step onboarding for complex entities

### Relations are mostly field-driven

Relations currently map well from specific structured fields, but there is still room for:

- richer relation editing
- multiple relations of the same kind
- relation metadata editing in UI
- timeline-aware or era-aware relations

### No domain-specific views yet

The plugin has strong schemas, but not many specialized worldbuilding views yet, such as:

- character roster
- faction directory
- location hierarchy explorer
- event timeline
- relationship matrix

## 11. Good directions for brainstorming

If the goal is "what should we add for worldbuilding entities?", these are the most promising brainstorming buckets.

### A. More entity types

Natural additions include:

- species / ancestry
- culture
- religion
- nation / kingdom / polity
- settlement
- landmark
- organization subtype taxonomy
- artifact / item
- magic system
- creature / monster
- profession / role
- language
- historical era
- conflict / war / campaign
- chapter / scene / story beat

### B. Richer relation types

Examples:

- parent_of / child_of
- rules / governed_by
- allied_with / hostile_to
- owns / controlled_by
- worships / patron_of
- originates_from
- successor_to / predecessor_to
- resides_in vs born_in vs died_in
- participated_in with stronger event-role modeling

### C. Better structured field types

Potential upgrades:

- multi-entity references
- repeatable sections
- rank/title fields
- numeric stats
- date ranges
- tags with controlled vocabularies
- image/portrait field
- map coordinates or region bindings

### D. Dedicated worldbuilding workflows

Potential UX improvements:

- "Create Character" top-level action outside the plugin modal
- entity-specific creation wizards
- duplicate-from-template or clone-entity flow
- suggested related entities after creation
- auto-create missing referenced entities

### E. Specialized views

High-value UI additions:

- character list/grid
- faction membership browser
- place hierarchy tree
- event timeline
- relation inspector
- world encyclopedia landing page

### F. Authoring helpers

Useful smart features:

- schema-aware autocomplete in frontmatter
- mention/reference picker for entity_ref fields
- validation for missing required fields
- warnings for broken references
- suggestions for reciprocal relations

## 12. Constraints to keep in mind while brainstorming

These are important guardrails for future design:

### Keep markdown as the source of truth

The current system is strongest when files stay understandable outside the app. New features should ideally still round-trip cleanly through markdown + frontmatter.

### Prefer schema-driven features first

Because the plugin is currently driven by TOML schemas and templates, the cheapest and safest improvements are usually:

- new entity types
- new relation types
- new field types
- better templating

before building large custom per-entity frontend experiences.

### Separate "entity metadata" from "narrative prose"

The current model already suggests a healthy split:

- frontmatter for structured facts
- note body for narrative detail

That should stay clear as the worldbuilding model grows.

### Design for partial adoption

Some users will model everything; others will only structure a few notes. The system should continue to work when a vault mixes:

- plain notes
- lightly structured notes
- fully typed entities

## 13. Questions worth asking Claude

If you want to brainstorm productively, these are strong prompt directions:

1. Given the current entity model, what are the next 10 worldbuilding entity types that would add the most value without making the schema too complex?
2. Which relation types should be added to support character, political, and historical storytelling better?
3. How should a worldbuilding plugin balance markdown flexibility with stronger structure?
4. What dedicated views would make this feel more like a worldbuilding tool and less like generic note-taking?
5. Which additions can remain schema-driven, and which ones likely need dedicated frontend UI?
6. What is the best way to model timelines, geography, and group membership without overcomplicating authoring?

## 14. Short summary

Codex's worldbuilding plugin is currently a **schema-and-template-driven entity system on top of markdown notes**. Its current strengths are typed entities, typed relations, and direct creation flows for characters, factions, locations, and events. The best near-term expansion area is likely **more entity types, richer relations, and a few dedicated worldbuilding views**, while keeping markdown files as the primary source of truth.
