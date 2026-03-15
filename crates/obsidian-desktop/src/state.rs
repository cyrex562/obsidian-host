use chrono::Local;
use iced::futures::lock::Mutex as AsyncMutex;
use iced::widget::{image, markdown};
use obsidian_client::ObsidianClient;
use obsidian_types::{
    EditorMode as PreferenceEditorMode, FileContent, FileNode, OrganizationSuggestion,
    OutlineSection, SearchResult, UserPreferences, Vault,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, fs, path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum EditorMode {
    Raw,
    Formatted,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileKind {
    Markdown,
    Image,
    Pdf,
    Text,
    Audio,
    Video,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ToolbarAction {
    Heading,
    Bold,
    Italic,
    BulletList,
    Quote,
    CodeFence,
}

#[derive(Clone)]
pub(crate) struct SharedWsStream(pub(crate) Arc<AsyncMutex<obsidian_client::WsStream>>);

impl std::fmt::Debug for SharedWsStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedWsStream(..)")
    }
}

/// Runtime feature flags — toggled in the diagnostics panel or via env vars at startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FeatureFlags {
    /// Show the ML outline / suggestion panels.
    pub(crate) ml_features: bool,
    /// Enable in-app media preview (images / PDF / audio / video).
    pub(crate) media_preview: bool,
    /// Allow WebSocket event-sync connection.
    pub(crate) event_sync: bool,
    /// Show the developer diagnostics panel.
    pub(crate) diagnostics_panel: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        // Each flag can be overridden by an env-var at startup
        // (e.g. OBSIDIAN_DISABLE_ML=1 turns off ml_features).
        Self {
            ml_features: std::env::var("OBSIDIAN_DISABLE_ML").is_err(),
            media_preview: std::env::var("OBSIDIAN_DISABLE_MEDIA").is_err(),
            event_sync: std::env::var("OBSIDIAN_DISABLE_SYNC").is_err(),
            diagnostics_panel: std::env::var("OBSIDIAN_DIAGNOSTICS").is_ok(),
        }
    }
}

/// Lightweight session-diagnostic counters (non-persisted).
#[derive(Debug, Clone, Default)]
pub(crate) struct DiagnosticsState {
    pub(crate) notes_loaded: u32,
    pub(crate) notes_saved: u32,
    pub(crate) ml_requests: u32,
    pub(crate) sync_messages_received: u32,
    pub(crate) errors_logged: u32,
    /// Scrollable log of recent events (capped at MAX_LOG_LINES).
    pub(crate) log_lines: Vec<String>,
}

impl DiagnosticsState {
    const MAX_LOG_LINES: usize = 200;

    pub(crate) fn push_log(&mut self, line: impl Into<String>) {
        if self.log_lines.len() >= Self::MAX_LOG_LINES {
            self.log_lines.remove(0);
        }
        self.log_lines.push(line.into());
    }

    /// Flatten all diagnostic state into a copyable string for the clipboard / bug reports.
    pub(crate) fn as_report(&self, flags: &FeatureFlags) -> String {
        format!(
            concat!(
                "=== Obsidian Desktop Diagnostics ===\n",
                "Feature flags:\n",
                "  ml_features       = {ml}\n",
                "  media_preview     = {media}\n",
                "  event_sync        = {sync}\n",
                "  diagnostics_panel = {diag}\n",
                "Session counters:\n",
                "  notes_loaded           = {nl}\n",
                "  notes_saved            = {ns}\n",
                "  ml_requests            = {mlr}\n",
                "  sync_messages_received = {smr}\n",
                "  errors_logged          = {el}\n",
                "Recent log ({ll} lines):\n{log}\n",
            ),
            ml = flags.ml_features,
            media = flags.media_preview,
            sync = flags.event_sync,
            diag = flags.diagnostics_panel,
            nl = self.notes_loaded,
            ns = self.notes_saved,
            mlr = self.ml_requests,
            smr = self.sync_messages_received,
            el = self.errors_logged,
            ll = self.log_lines.len(),
            log = self.log_lines.join("\n"),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemplateInsertMode {
    Append,
    Replace,
}

/// Which style of server connection the desktop client should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum DesktopMode {
    /// Connect to a remote cloud server only (full API, no local file access).
    #[default]
    Cloud,
    /// The server is local; treat file paths as local-disk paths.
    Standalone,
    /// Cloud URL for all API calls + optional local mirror URL for faster binary reads.
    Hybrid,
}

#[derive(Debug)]
pub(crate) struct DesktopApp {
    pub(crate) base_url: String,
    pub(crate) username: String,
    pub(crate) password: String,
    /// Which connection mode to use when logging in.
    pub(crate) deployment_mode: DesktopMode,
    /// Secondary (local mirror) base URL used only in Hybrid mode.
    pub(crate) local_base_url: String,
    /// Pending name for vault creation.
    pub(crate) new_vault_name: String,
    pub(crate) status: String,

    /// Runtime feature toggle flags.
    pub(crate) feature_flags: FeatureFlags,
    /// Session-scoped diagnostic counters and log ring-buffer.
    pub(crate) diagnostics: DiagnosticsState,

    pub(crate) client: Option<ObsidianClient>,
    pub(crate) vaults: Vec<Vault>,
    pub(crate) selected_vault_id: Option<String>,
    pub(crate) event_sync_requested: bool,
    pub(crate) event_sync_connected: bool,
    pub(crate) event_sync_retry_attempt: u32,
    pub(crate) event_sync_last_message: String,
    pub(crate) event_sync_stream: Option<SharedWsStream>,
    pub(crate) preferences_visible: bool,
    pub(crate) preferences_theme: String,
    pub(crate) preferences_editor_mode: PreferenceEditorMode,
    pub(crate) preferences_font_size_input: String,
    pub(crate) preferences_window_layout_input: String,
    pub(crate) preferences_icon_map_raw: String,
    pub(crate) pending_session_tab_paths: Vec<String>,
    pub(crate) session_restore_in_progress: bool,

    pub(crate) tree_entries: Vec<TreeEntry>,
    pub(crate) recent_files: Vec<String>,
    pub(crate) quick_switcher_query: String,
    pub(crate) search_query: String,
    pub(crate) search_results: Vec<SearchResult>,
    pub(crate) search_total_count: usize,
    pub(crate) search_page: usize,
    pub(crate) search_page_size: usize,
    pub(crate) outline_sections: Vec<OutlineSection>,
    pub(crate) outline_summary: String,
    pub(crate) outline_max_sections: usize,
    pub(crate) suggestion_items: Vec<OrganizationSuggestion>,
    pub(crate) suggestion_max_count: usize,
    pub(crate) suggestion_source_path: String,
    pub(crate) last_ml_receipt_id: Option<String>,
    pub(crate) last_ml_action_summary: String,
    pub(crate) outgoing_links: Vec<String>,
    pub(crate) backlink_paths: Vec<String>,
    pub(crate) tag_entries: Vec<TagPanelEntry>,
    pub(crate) bookmarks: Vec<String>,
    pub(crate) selected_tree_path: Option<String>,
    pub(crate) open_tabs: Vec<OpenTab>,
    pub(crate) active_tab_path: Option<String>,

    pub(crate) note_path: String,
    pub(crate) new_file_path: String,
    pub(crate) new_folder_path: String,
    pub(crate) rename_from_path: String,
    pub(crate) rename_to_path: String,
    pub(crate) delete_target_path: String,
    pub(crate) delete_confirmation_armed: bool,
    pub(crate) template_path: String,
    pub(crate) template_insert_mode: TemplateInsertMode,
    pub(crate) editor_mode: EditorMode,
    pub(crate) note_content: String,
    pub(crate) note_frontmatter_raw: String,
    pub(crate) preview_content: String,
    pub(crate) preview_markdown: Vec<markdown::Item>,
    pub(crate) rendered_preview_html: String,
    pub(crate) preview_render_error: Option<String>,
    pub(crate) note_modified: Option<String>,
    pub(crate) note_frontmatter_summary: String,
    pub(crate) note_is_dirty: bool,
    pub(crate) conflict_active: bool,
    pub(crate) conflict_message: String,
    pub(crate) current_file_kind: FileKind,
    pub(crate) media_source_url: String,
    pub(crate) media_image: Option<image::Handle>,
    pub(crate) media_status: String,

    pub(crate) plugin_panel_visible: bool,
    pub(crate) plugins: Vec<PluginItem>,
    pub(crate) plugin_status: String,

    pub(crate) import_export_visible: bool,
    pub(crate) import_local_path: String,
    pub(crate) import_vault_path: String,
    pub(crate) import_status: String,
    pub(crate) export_vault_path: String,
    pub(crate) export_local_path: String,
    pub(crate) export_status: String,
}

impl Default for DesktopApp {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            username: String::new(),
            password: String::new(),
            deployment_mode: DesktopMode::default(),
            local_base_url: String::new(),
            new_vault_name: String::new(),
            status: String::new(),
            feature_flags: FeatureFlags::default(),
            diagnostics: DiagnosticsState::default(),
            client: None,
            vaults: Vec::new(),
            selected_vault_id: None,
            event_sync_requested: false,
            event_sync_connected: false,
            event_sync_retry_attempt: 0,
            event_sync_last_message: "Disconnected".to_string(),
            event_sync_stream: None,
            preferences_visible: false,
            preferences_theme: "dark".to_string(),
            preferences_editor_mode: PreferenceEditorMode::SideBySide,
            preferences_font_size_input: "14".to_string(),
            preferences_window_layout_input: String::new(),
            preferences_icon_map_raw: String::new(),
            pending_session_tab_paths: Vec::new(),
            session_restore_in_progress: false,
            tree_entries: Vec::new(),
            recent_files: Vec::new(),
            quick_switcher_query: String::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_total_count: 0,
            search_page: 1,
            search_page_size: 20,
            outline_sections: Vec::new(),
            outline_summary: String::new(),
            outline_max_sections: 24,
            suggestion_items: Vec::new(),
            suggestion_max_count: 8,
            suggestion_source_path: String::new(),
            last_ml_receipt_id: None,
            last_ml_action_summary: String::new(),
            outgoing_links: Vec::new(),
            backlink_paths: Vec::new(),
            tag_entries: Vec::new(),
            bookmarks: Vec::new(),
            selected_tree_path: None,
            open_tabs: Vec::new(),
            active_tab_path: None,
            note_path: String::new(),
            new_file_path: String::new(),
            new_folder_path: String::new(),
            rename_from_path: String::new(),
            rename_to_path: String::new(),
            delete_target_path: String::new(),
            delete_confirmation_armed: false,
            template_path: "Templates/Daily Note.md".to_string(),
            template_insert_mode: TemplateInsertMode::Append,
            editor_mode: EditorMode::Formatted,
            note_content: String::new(),
            note_frontmatter_raw: String::new(),
            preview_content: String::new(),
            preview_markdown: Vec::new(),
            rendered_preview_html: String::new(),
            preview_render_error: None,
            note_modified: None,
            note_frontmatter_summary: "Frontmatter: none".to_string(),
            note_is_dirty: false,
            conflict_active: false,
            conflict_message: String::new(),
            current_file_kind: FileKind::Markdown,
            media_source_url: String::new(),
            media_image: None,
            media_status: String::new(),
            plugin_panel_visible: false,
            plugins: Vec::new(),
            plugin_status: String::new(),
            import_export_visible: false,
            import_local_path: String::new(),
            import_vault_path: String::new(),
            import_status: String::new(),
            export_vault_path: String::new(),
            export_local_path: String::new(),
            export_status: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TreeEntry {
    pub(crate) path: String,
    pub(crate) display: String,
    pub(crate) is_directory: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct OpenTab {
    pub(crate) path: String,
    pub(crate) title: String,
    pub(crate) file_kind: FileKind,
    pub(crate) content: String,
    pub(crate) frontmatter_raw: String,
    pub(crate) modified: Option<String>,
    pub(crate) frontmatter_summary: String,
    pub(crate) is_dirty: bool,
    pub(crate) media_source_url: String,
    pub(crate) media_image: Option<image::Handle>,
}

#[derive(Debug, Clone)]
pub(crate) struct TagPanelEntry {
    pub(crate) tag: String,
    pub(crate) count: usize,
    pub(crate) files: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PluginItem {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) enabled: bool,
    pub(crate) state_label: String,
    pub(crate) last_error: Option<String>,
}

pub(crate) fn parse_plugin_items(values: &[serde_json::Value]) -> Vec<PluginItem> {
    values
        .iter()
        .filter_map(|v| {
            let manifest = v.get("manifest")?;
            let id = manifest.get("id")?.as_str()?.to_string();
            let name = manifest.get("name")?.as_str()?.to_string();
            let version = manifest.get("version")?.as_str()?.to_string();
            let description = manifest
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            let enabled = v.get("enabled").and_then(|e| e.as_bool()).unwrap_or(false);
            let state_label = v
                .get("state")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown")
                .to_string();
            let last_error = v
                .get("last_error")
                .and_then(|e| e.as_str())
                .map(|s| s.to_string());
            Some(PluginItem {
                id,
                name,
                version,
                description,
                enabled,
                state_label,
                last_error,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedSessionState {
    pub(crate) selected_vault_id: Option<String>,
    pub(crate) open_tab_paths: Vec<String>,
    pub(crate) active_tab_path: Option<String>,
    pub(crate) selected_tree_path: Option<String>,
    pub(crate) note_path: Option<String>,
    pub(crate) editor_mode: EditorMode,
    // Connection settings — added later so #[serde(default)] keeps old snapshots valid.
    #[serde(default)]
    pub(crate) base_url: Option<String>,
    #[serde(default)]
    pub(crate) username: Option<String>,
    #[serde(default)]
    pub(crate) deployment_mode: Option<DesktopMode>,
    #[serde(default)]
    pub(crate) local_base_url: Option<String>,
}

pub(crate) fn flatten_tree(nodes: &[FileNode]) -> Vec<TreeEntry> {
    fn walk(node: &FileNode, depth: usize, out: &mut Vec<TreeEntry>) {
        let prefix = "  ".repeat(depth);
        out.push(TreeEntry {
            path: node.path.clone(),
            display: format!("{prefix}{}", node.name),
            is_directory: node.is_directory,
        });

        if let Some(children) = &node.children {
            for child in children {
                walk(child, depth + 1, out);
            }
        }
    }

    let mut out = Vec::new();
    for node in nodes {
        walk(node, 0, &mut out);
    }
    out
}

pub(crate) fn summarize_frontmatter(file: &FileContent) -> String {
    summarize_frontmatter_value(file.frontmatter.as_ref())
}

pub(crate) fn file_kind_from_path(path: &str) -> FileKind {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" => FileKind::Markdown,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => FileKind::Image,
        "pdf" => FileKind::Pdf,
        "mp3" | "wav" | "ogg" => FileKind::Audio,
        "mp4" | "webm" | "mov" | "ogv" => FileKind::Video,
        "txt" | "json" | "js" | "ts" | "css" | "html" | "xml" | "rs" | "py" | "java" | "c"
        | "cpp" | "h" | "go" | "yaml" | "yml" | "toml" | "ini" | "sh" | "bat" | "mdx" => {
            FileKind::Text
        }
        _ => FileKind::Other,
    }
}

pub(crate) fn file_kind_label(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Markdown => "Markdown",
        FileKind::Image => "Image",
        FileKind::Pdf => "PDF",
        FileKind::Text => "Text",
        FileKind::Audio => "Audio",
        FileKind::Video => "Video",
        FileKind::Other => "Binary",
    }
}

pub(crate) fn is_media_file_kind(kind: FileKind) -> bool {
    matches!(
        kind,
        FileKind::Image | FileKind::Pdf | FileKind::Audio | FileKind::Video | FileKind::Other
    )
}

pub(crate) fn summarize_frontmatter_value(frontmatter: Option<&Value>) -> String {
    match frontmatter {
        Some(frontmatter) => {
            if let Some(map) = frontmatter.as_object() {
                if !map.is_empty() {
                    return format!(
                        "Frontmatter fields: {}",
                        map.keys().cloned().collect::<Vec<_>>().join(", ")
                    );
                }
            }

            "Frontmatter: present".to_string()
        }
        None => "Frontmatter: none".to_string(),
    }
}

pub(crate) fn format_frontmatter(frontmatter: Option<&Value>) -> String {
    match frontmatter {
        Some(value) => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
        None => String::new(),
    }
}

pub(crate) fn parse_frontmatter(raw: &str) -> Result<Option<Value>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let value: Value = serde_json::from_str(trimmed)
        .map_err(|err| format!("Frontmatter must be valid JSON: {err}"))?;

    if !value.is_object() {
        return Err("Frontmatter JSON must be an object".to_string());
    }

    Ok(Some(value))
}

pub(crate) fn apply_file_to_state(state: &mut DesktopApp, file: FileContent) {
    let frontmatter_summary = summarize_frontmatter(&file);
    let frontmatter_raw = format_frontmatter(file.frontmatter.as_ref());
    let path = file.path.clone();
    let title = tab_title_from_path(&path);
    let modified = Some(file.modified.to_rfc3339());
    let content = file.content;
    let file_kind = file_kind_from_path(&path);

    upsert_open_tab(
        state,
        OpenTab {
            path: path.clone(),
            title,
            file_kind,
            content: content.clone(),
            frontmatter_raw: frontmatter_raw.clone(),
            modified: modified.clone(),
            frontmatter_summary: frontmatter_summary.clone(),
            is_dirty: false,
            media_source_url: String::new(),
            media_image: None,
        },
    );

    state.note_path = path.clone();
    state.selected_tree_path = Some(path.clone());
    state.active_tab_path = Some(path);
    state.rename_from_path = state.note_path.clone();
    state.delete_target_path = state.note_path.clone();
    state.note_content = content.clone();
    state.note_frontmatter_raw = frontmatter_raw;
    state.preview_content = content;
    refresh_preview_markdown(state);
    state.rendered_preview_html.clear();
    state.preview_render_error = None;
    state.note_modified = modified;
    state.note_frontmatter_summary = frontmatter_summary;
    state.note_is_dirty = false;
    state.conflict_active = false;
    state.conflict_message.clear();
    state.current_file_kind = file_kind;
    state.media_source_url.clear();
    state.media_image = None;
    state.media_status.clear();
    state.outline_sections.clear();
    state.outline_summary.clear();
    state.suggestion_items.clear();
    state.suggestion_source_path.clear();
    state.last_ml_receipt_id = None;
    state.last_ml_action_summary.clear();
    state.outgoing_links.clear();
    state.backlink_paths.clear();

    add_recent_file(state, &state.note_path.clone());
}

pub(crate) fn apply_media_to_state(
    state: &mut DesktopApp,
    path: String,
    file_kind: FileKind,
    media_source_url: String,
    media_image: Option<image::Handle>,
) {
    let title = tab_title_from_path(&path);
    upsert_open_tab(
        state,
        OpenTab {
            path: path.clone(),
            title,
            file_kind,
            content: String::new(),
            frontmatter_raw: String::new(),
            modified: None,
            frontmatter_summary: format!("{} file", file_kind_label(file_kind)),
            is_dirty: false,
            media_source_url: media_source_url.clone(),
            media_image: media_image.clone(),
        },
    );

    state.note_path = path.clone();
    state.selected_tree_path = Some(path.clone());
    state.active_tab_path = Some(path.clone());
    state.rename_from_path = path.clone();
    state.delete_target_path = path.clone();
    state.note_content.clear();
    state.note_frontmatter_raw.clear();
    state.preview_content.clear();
    state.preview_markdown.clear();
    state.rendered_preview_html.clear();
    state.preview_render_error = None;
    state.note_modified = None;
    state.note_frontmatter_summary = format!("{} file", file_kind_label(file_kind));
    state.note_is_dirty = false;
    state.conflict_active = false;
    state.conflict_message.clear();
    state.current_file_kind = file_kind;
    state.media_source_url = media_source_url;
    state.media_image = media_image;
    state.media_status = format!("Loaded {}", file_kind_label(file_kind));
    state.outline_sections.clear();
    state.outline_summary.clear();
    state.suggestion_items.clear();
    state.suggestion_source_path.clear();
    state.last_ml_receipt_id = None;
    state.last_ml_action_summary.clear();
    state.outgoing_links.clear();
    state.backlink_paths.clear();

    add_recent_file(state, &path);
}

pub(crate) fn sync_rename_source_from_selection(state: &mut DesktopApp) {
    let candidate = state
        .selected_tree_path
        .clone()
        .filter(|path| !path.is_empty())
        .or_else(|| {
            state
                .active_tab_path
                .clone()
                .filter(|path| !path.is_empty())
        });

    if let Some(path) = candidate {
        state.rename_from_path = path;
        state.delete_target_path = state.rename_from_path.clone();
    }
}

pub(crate) fn apply_rename_to_state(state: &mut DesktopApp, from: &str, to: &str) {
    let remap = |path: &str| remap_path(path, from, to);

    if let Some(selected_path) = state.selected_tree_path.clone() {
        state.selected_tree_path = remap(&selected_path);
    }

    if let Some(active_path) = state.active_tab_path.clone() {
        state.active_tab_path = remap(&active_path);
    }

    state.note_path = remap(&state.note_path).unwrap_or_else(|| state.note_path.clone());
    state.rename_from_path = remap(&state.rename_from_path).unwrap_or_else(|| to.to_string());
    state.delete_target_path =
        remap(&state.delete_target_path).unwrap_or_else(|| state.delete_target_path.clone());

    for tab in &mut state.open_tabs {
        if let Some(updated_path) = remap(&tab.path) {
            tab.path = updated_path.clone();
            tab.title = tab_title_from_path(&updated_path);
        }
    }
}

pub(crate) fn apply_delete_to_state(state: &mut DesktopApp, target: &str) {
    state
        .recent_files
        .retain(|path| !path_matches_target(path, target));

    state
        .open_tabs
        .retain(|tab| !path_matches_target(&tab.path, target));

    if state
        .selected_tree_path
        .as_deref()
        .is_some_and(|path| path_matches_target(path, target))
    {
        state.selected_tree_path = None;
    }

    if path_matches_target(&state.note_path, target) {
        clear_loaded_note(state);
        state.active_tab_path = None;
    }

    if state
        .active_tab_path
        .as_deref()
        .is_some_and(|path| path_matches_target(path, target))
    {
        state.active_tab_path = None;
    }

    state.delete_target_path.clear();
    state.delete_confirmation_armed = false;

    if state.active_tab_path.is_none() {
        if let Some(next_path) = state.open_tabs.last().map(|tab| tab.path.clone()) {
            state.active_tab_path = Some(next_path.clone());
            state.selected_tree_path = Some(next_path.clone());
            let _ = activate_existing_tab(state, &next_path);
        }
    }
}

pub(crate) fn add_recent_file(state: &mut DesktopApp, path: &str) {
    state.recent_files.retain(|existing| existing != path);
    state.recent_files.insert(0, path.to_string());
    state.recent_files.truncate(10);
}

pub(crate) fn apply_toolbar_action(content: &str, action: ToolbarAction) -> String {
    let snippet = match action {
        ToolbarAction::Heading => "# New heading",
        ToolbarAction::Bold => "**bold text**",
        ToolbarAction::Italic => "*italic text*",
        ToolbarAction::BulletList => "- list item",
        ToolbarAction::Quote => "> quote",
        ToolbarAction::CodeFence => "```\ncode\n```",
    };

    if content.trim().is_empty() {
        return snippet.to_string();
    }

    format!("{}\n\n{snippet}", content.trim_end())
}

pub(crate) fn process_template_content(
    template_raw: &str,
    note_path: &str,
    current_content: &str,
    mode: TemplateInsertMode,
) -> String {
    let now = Local::now();
    let note_title = tab_title_from_path(note_path);
    let title = note_title
        .strip_suffix(".md")
        .unwrap_or(note_title.as_str())
        .to_string();

    let mut processed = template_raw.to_string();
    let replacements = [
        ("{{date}}", now.format("%Y-%m-%d").to_string()),
        ("{{time}}", now.format("%H:%M:%S").to_string()),
        ("{{datetime}}", now.format("%Y-%m-%d %H:%M:%S").to_string()),
        ("{{year}}", now.format("%Y").to_string()),
        ("{{month}}", now.format("%m").to_string()),
        ("{{day}}", now.format("%d").to_string()),
        ("{{day-num}}", now.format("%-d").to_string()),
        ("{{day-name}}", now.format("%A").to_string()),
        ("{{month-name}}", now.format("%B").to_string()),
        ("{{title}}", title),
        ("{{selection}}", String::new()),
        ("{{clipboard}}", String::new()),
        ("{{cursor}}", String::new()),
    ];

    for (key, value) in replacements {
        processed = processed.replace(key, &value);
    }

    match mode {
        TemplateInsertMode::Replace => processed,
        TemplateInsertMode::Append => {
            if current_content.trim().is_empty() {
                processed
            } else {
                format!(
                    "{}\n\n{}",
                    current_content.trim_end(),
                    processed.trim_start()
                )
            }
        }
    }
}

fn path_matches_target(path: &str, target: &str) -> bool {
    path == target || path.starts_with(&format!("{target}/"))
}

fn remap_path(path: &str, from: &str, to: &str) -> Option<String> {
    if path == from {
        return Some(to.to_string());
    }

    let prefix = format!("{from}/");
    path.strip_prefix(&prefix)
        .map(|suffix| format!("{to}/{suffix}"))
}

pub(crate) fn tab_title_from_path(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

pub(crate) fn upsert_open_tab(state: &mut DesktopApp, new_tab: OpenTab) {
    if let Some(existing) = state
        .open_tabs
        .iter_mut()
        .find(|tab| tab.path == new_tab.path)
    {
        *existing = new_tab;
    } else {
        state.open_tabs.push(new_tab);
    }
}

pub(crate) fn activate_existing_tab(state: &mut DesktopApp, path: &str) -> bool {
    let Some(tab) = state.open_tabs.iter().find(|tab| tab.path == path).cloned() else {
        return false;
    };

    state.active_tab_path = Some(tab.path.clone());
    state.note_path = tab.path.clone();
    state.current_file_kind = tab.file_kind;
    state.media_source_url = tab.media_source_url.clone();
    state.media_image = tab.media_image.clone();

    if is_media_file_kind(tab.file_kind) {
        state.note_content.clear();
        state.note_frontmatter_raw.clear();
        state.preview_content.clear();
        state.preview_markdown.clear();
        state.rendered_preview_html.clear();
        state.preview_render_error = None;
        state.note_modified = None;
        state.note_frontmatter_summary = tab.frontmatter_summary;
        state.note_is_dirty = false;
        state.media_status = format!("Loaded {}", file_kind_label(tab.file_kind));
    } else {
        state.note_content = tab.content.clone();
        state.note_frontmatter_raw = tab.frontmatter_raw.clone();
        state.preview_content = tab.content;
        refresh_preview_markdown(state);
        state.note_modified = tab.modified.clone();
        state.note_frontmatter_summary = tab.frontmatter_summary;
        state.note_is_dirty = tab.is_dirty;
        state.media_status.clear();
    }
    true
}

pub(crate) fn refresh_preview_markdown(state: &mut DesktopApp) {
    state.preview_markdown = markdown::parse(&state.preview_content).collect();
}

pub(crate) fn sync_active_tab_from_editor(state: &mut DesktopApp) {
    let Some(active_path) = state.active_tab_path.clone() else {
        return;
    };

    if let Some(tab) = state
        .open_tabs
        .iter_mut()
        .find(|tab| tab.path == active_path)
    {
        tab.file_kind = state.current_file_kind;
        tab.content = state.note_content.clone();
        tab.frontmatter_raw = state.note_frontmatter_raw.clone();
        tab.modified = state.note_modified.clone();
        tab.frontmatter_summary = state.note_frontmatter_summary.clone();
        tab.is_dirty = state.note_is_dirty;
        tab.media_source_url = state.media_source_url.clone();
        tab.media_image = state.media_image.clone();
    }
}

pub(crate) fn clear_loaded_note(state: &mut DesktopApp) {
    state.note_path.clear();
    state.note_content.clear();
    state.note_frontmatter_raw.clear();
    state.preview_content.clear();
    state.preview_markdown.clear();
    state.rendered_preview_html.clear();
    state.preview_render_error = None;
    state.note_modified = None;
    state.note_frontmatter_summary = "Frontmatter: none".to_string();
    state.note_is_dirty = false;
    state.conflict_active = false;
    state.conflict_message.clear();
    state.current_file_kind = FileKind::Markdown;
    state.media_source_url.clear();
    state.media_image = None;
    state.media_status.clear();
    state.outline_sections.clear();
    state.outline_summary.clear();
    state.suggestion_items.clear();
    state.suggestion_source_path.clear();
    state.last_ml_receipt_id = None;
    state.last_ml_action_summary.clear();
    state.outgoing_links.clear();
    state.backlink_paths.clear();
}

pub(crate) fn apply_preferences_to_state(state: &mut DesktopApp, prefs: &UserPreferences) {
    state.preferences_theme = prefs.theme.clone();
    state.preferences_editor_mode = prefs.editor_mode.clone();
    state.preferences_font_size_input = prefs.font_size.to_string();
    state.preferences_window_layout_input = prefs.window_layout.clone().unwrap_or_default();
    state.preferences_icon_map_raw = prefs
        .icon_map
        .as_ref()
        .and_then(|map| serde_json::to_string_pretty(map).ok())
        .unwrap_or_default();

    state.editor_mode = match prefs.editor_mode {
        PreferenceEditorMode::Raw => EditorMode::Raw,
        PreferenceEditorMode::FullyRendered => EditorMode::Preview,
        PreferenceEditorMode::SideBySide | PreferenceEditorMode::FormattedRaw => {
            EditorMode::Formatted
        }
    };
}

pub(crate) fn capture_session_state(state: &DesktopApp) -> PersistedSessionState {
    PersistedSessionState {
        selected_vault_id: state.selected_vault_id.clone(),
        open_tab_paths: state.open_tabs.iter().map(|tab| tab.path.clone()).collect(),
        active_tab_path: state.active_tab_path.clone(),
        selected_tree_path: state.selected_tree_path.clone(),
        note_path: (!state.note_path.trim().is_empty()).then(|| state.note_path.clone()),
        editor_mode: state.editor_mode,
        base_url: (!state.base_url.trim().is_empty()).then(|| state.base_url.clone()),
        username: (!state.username.trim().is_empty()).then(|| state.username.clone()),
        deployment_mode: Some(state.deployment_mode),
        local_base_url: (!state.local_base_url.trim().is_empty())
            .then(|| state.local_base_url.clone()),
    }
}

pub(crate) fn apply_restored_session(state: &mut DesktopApp, session: PersistedSessionState) {
    state.selected_vault_id = session.selected_vault_id;
    state.selected_tree_path = session.selected_tree_path;
    state.note_path = session.note_path.unwrap_or_default();
    state.editor_mode = session.editor_mode;
    if let Some(url) = session.base_url {
        state.base_url = url;
    }
    if let Some(un) = session.username {
        state.username = un;
    }
    if let Some(mode) = session.deployment_mode {
        state.deployment_mode = mode;
    }
    if let Some(local_url) = session.local_base_url {
        state.local_base_url = local_url;
    }

    let preferred_path = session
        .active_tab_path
        .clone()
        .or_else(|| (!state.note_path.is_empty()).then(|| state.note_path.clone()));

    let mut pending_paths = session.open_tab_paths;
    pending_paths.retain(|path| !path.trim().is_empty());
    pending_paths.sort();
    pending_paths.dedup();

    if let Some(preferred_path) = preferred_path {
        if let Some(index) = pending_paths
            .iter()
            .position(|path| path == &preferred_path)
        {
            let active = pending_paths.remove(index);
            pending_paths.insert(0, active);
        } else {
            pending_paths.insert(0, preferred_path);
        }
    }

    state.pending_session_tab_paths = pending_paths;
    state.session_restore_in_progress = !state.pending_session_tab_paths.is_empty();
}

pub(crate) fn persist_session_state(state: &DesktopApp) -> Result<(), String> {
    let path = session_state_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create session state directory: {err}"))?;
    }

    let json = serde_json::to_string_pretty(&capture_session_state(state))
        .map_err(|err| format!("Failed to serialize session state: {err}"))?;
    fs::write(&path, json).map_err(|err| format!("Failed to write session state: {err}"))
}

pub(crate) fn load_persisted_session() -> Result<Option<PersistedSessionState>, String> {
    let path = session_state_file_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read session state file: {err}"))?;
    let session = serde_json::from_str(&raw)
        .map_err(|err| format!("Failed to parse session state file: {err}"))?;
    Ok(Some(session))
}

fn session_state_file_path() -> Result<PathBuf, String> {
    let base_dir = if let Ok(app_data) = env::var("APPDATA") {
        PathBuf::from(app_data)
    } else if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg_config)
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        env::current_dir().map_err(|err| format!("Failed to resolve current directory: {err}"))?
    };

    Ok(base_dir.join("obsidian-host").join("desktop-session.json"))
}
