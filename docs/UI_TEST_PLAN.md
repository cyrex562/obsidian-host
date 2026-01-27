# Obsidian Host UI Test Plan

## Overview

This document outlines the manual and automated testing strategy to verify all user interface features of the Obsidian Host application. It covers core functionality, plugin management, and advanced features.

## 1. Vault Management

### 1.1 Vault Selection & Creation

- [x] **Initial State**: Verify "Select a vault..." is shown in the top bar dropdown.
- [x] **Add Vault**:
  - Click "Add Vault" button.
  - Verify modal appears.
  - Enter valid path `c:\temp\test_vault`.
  - Click Save.
  - Verify vault appears in dropdown and is automatically selected.
- [x] **Switch Vault**:
  - Add a second vault.
  - Switch between Vault A and Vault B using the dropdown.
  - Verify file tree refreshes to show correct content for each.

### 1.2 Connection Status

- [x] **Online State**: Verify green dot / "Connected" status when server is running.
- [x] **Offline State**: Stop server. Verify red dot / "Disconnected" status appears within 5 seconds.
- [x] **Reconnection**: Restart server. Verify status returns to green automatically.

## 2. File Operations (Sidebar)

### 2.1 File Tree Navigation

- [x] **Expand/Collapse**: Click folder arrows to toggle visibility of children.
- [x] **Selection**: Click a file to highlight it in the tree.
- [x] **Empty Vault**: Verify "No files found" message for empty folders.

### 2.2 Context Menu Actions

- [x] **New File**: Right-click folder -> "New File". Verify input prompt appears. Enter name. Verify file opens.
- [x] **New Folder**: Right-click folder -> "New Folder". Verify folder created.
- [x] **Rename**: Right-click file -> "Rename". Rename `note.md` to `renamed.md`. Verify tree updates.
- [x] **Delete**: Right-click file -> "Delete". Confirm dialog. Verify file removed.
- [ ] **Add to Canvas**: (Feature 17) Verify context search option exists.

### 2.3 Drag and Drop

- [x] **Move File**: Drag `note.md` into `Archive/` folder. Verify path update.
- [x] **File Upload**: Drag a `.md` file from OS desktop to file tree. Verify upload success toast.

## 3. Editor Features

### 3.1 Content Editing

- [x] **Open File**: Click file in tree. Verify content loads in main editor.
- [x] **Typing**: specific text input. Verify "Unsaved changes" indicator.
- [x] **Auto-Save**: Wait 2 seconds. Verify "Saved" indicator appears.
- [x] **Markdown Rendering**:
  - Type `**bold**`: Verify bold text in preview.
  - Type `[[Internal Link]]`: Verify link creation.

### 3.2 Tabs & Layout

- [x] **Multi-Tab**: Ctrl+Click file. Verify it opens in new tab.
- [x] **Switch Tab**: Click tab header. Verify active editor changes.
- [x] **Close Tab**: Click `x` on tab. Verify tab closes.

### 3.3 Media Embedding

- [x] **Image**: Type `![[image.png]]`. Verify image renders if it exists in vault.

## 4. Search & Navigation (Feature 19)

### 4.1 Quick Switcher

- [x] **Open**: Press `Ctrl+K`. Verify modal appears.
- [x] **Filter**: Type "daily". Verify list filters to matching files (fuzzy match).
- [x] **Navigation**: Press Down Arrow -> Enter. Verify selected file opens.

### 4.2 Global Search

- [x] **Input**: Type "todo" in top search bar.
- [x] **Results**: Verify "Search Results" sidebar panel opens.
- [x] **Click**: Click a result. Verify editor scrolls to match.
- [ ] **Advanced Syntax**: Test `tag:#urgent` (Task 19.1). Verify only tagged notes appear.

## 5. Plugin System (Feature 15)

### 5.1 Plugin Manager UI

- [x] **Open**: Click 🧩 icon. Verify "Plugin Manager" modal opens.
- [x] **Installed List**: Verify Core Plugins (Daily Notes, Word Count) are listed.
- [x] **Status Badges**: Verify plugins show "Loaded" (Green).

### 5.2 Plugin Actions

- [x] **Toggle**: Click "Disable" on Word Count. Verify status changes to Grey.
  - Verify Word Count status bar item disappears.
- [x] **Enable**: Click "Enable". Verify status bar item reappears.
- [x] **Settings**: Click "Settings" icon. Verify configuration form appears.

### 5.3 Core Plugins Verification

- [x] **Daily Notes**: Click Calendar icon in sidebar. Verify today's note opens.
- [x] **Word Count**: Open a note. Type text. Verify counter updates in status bar.
- [x] **Backlinks**: Open Sidebar -> Backlinks tab. Verify incoming links are listed.

## 6. Advanced Views (Features 16-18)

### 6.1 Random Note

- [x] **Click**: Click Dice icon (Task 16.2). Verify a random note opens.

### 6.2 Metadata Editor (Feature 18)

- [x] **View**: Open note with frontmatter. Verify "Properties" panel shows key-value pairs.
- [x] **Edit**: Change a tag from `#wip` to `#done`. Verify file content updates.

### 6.3 Canvas (Feature 17)

- [x] **Create**: Create `New Canvas.canvas`. Verify graphical editor loads.
- [x] **Add Node**: Double click background. Start typing. Verify text node creation.

## 7. Responsive Design

### 7.1 Mobile View

- [ ] **Viewport**: Resize browser to 375px width.
- [ ] **Sidebar**: Verify sidebar collapses into hamburger menu.
- [ ] **Editor**: Verify text is readable and wrapped correcty.

## 8. Theme System

### 8.1 Dark/Light Mode

- [x] **Toggle**: Click Moon/Sun icon.
- [x] **Verify**: Background color changes (Dark `#1e1e1e` <-> Light `#ffffff`).
- [x] **Persistence**: Refresh page. Verify theme selection is remembered.

## Test Execution Log template

| Test ID | Feature | Status | Notes |
| :--- | :--- | :--- | :--- |
| 1.1 | Add Vault | Passed | UI Verification Successful. Backend API also verified. |
| 2.1 | File Tree | | |
| ... | ... | | |
