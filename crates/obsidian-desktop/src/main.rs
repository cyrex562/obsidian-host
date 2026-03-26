mod state;
mod ui;

use chrono::{DateTime, Utc};
use iced::futures::StreamExt;
use iced::{event, keyboard, Event, Subscription, Task, Theme};
use std::{process::Command, sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use obsidian_client::FinishUploadSessionRequest;
use obsidian_client::ObsidianClient;
use obsidian_types::{
    CreateUploadSessionRequest, CreateVaultRequest, EditorMode as PreferenceEditorMode,
    FileChangeType, FileContent, FileNode, PagedSearchResult, UserPreferences, Vault, WsMessage,
};

use state::{
    activate_existing_tab, add_recent_file, apply_delete_to_state, apply_file_to_state,
    apply_media_to_state, apply_preferences_to_state, apply_rename_to_state,
    apply_restored_session, apply_toolbar_action, clear_loaded_note, file_kind_from_path,
    file_kind_label, flatten_tree, is_media_file_kind, load_persisted_session, parse_frontmatter,
    parse_plugin_items, persist_session_state, process_template_content, refresh_preview_markdown,
    summarize_frontmatter_value, sync_active_tab_from_editor, sync_rename_source_from_selection,
    DesktopApp, DesktopMode, EditorMode, FileKind, PluginItem, SharedWsStream, TemplateInsertMode,
    ToolbarAction,
};

type MediaLoadResult = (String, FileKind, String, Option<Vec<u8>>);

fn main() -> iced::Result {
    // Initialise structured logging.  Set RUST_LOG=obsidian_desktop=debug for verbose output.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("obsidian_desktop=info")),
        )
        .with_target(false)
        .init();

    info!("Obsidian Desktop starting up");

    iced::application("Obsidian Desktop (Iced) Skeleton", update, ui::view)
        .theme(|_| Theme::TokyoNight)
        .subscription(subscription)
        .run_with(|| {
            (
                DesktopApp::default(),
                Task::done(Message::LoadLocalSessionPressed),
            )
        })
}

#[derive(Debug, Clone)]
enum Message {
    LoadLocalSessionPressed,
    LocalSessionLoaded(Result<Option<state::PersistedSessionState>, String>),
    WindowEventOccurred(Event),
    BaseUrlChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    LoginPressed,
    LoginFinished(Result<ObsidianClient, String>),

    LoadVaultsPressed,
    VaultsLoaded(Result<Vec<Vault>, String>),
    VaultSelected(String),
    PreferencesPressed,
    PreferencesClosed,
    PreferencesLoaded(Result<UserPreferences, String>),
    PreferencesThemeChanged(String),
    PreferencesFontSizeChanged(String),
    PreferencesWindowLayoutChanged(String),
    PreferencesEditorModeSelected(PreferenceEditorMode),
    PreferencesSavePressed,
    PreferencesSaved(Result<UserPreferences, String>),
    PreferencesResetPressed,
    PreferencesResetLoaded(Result<UserPreferences, String>),
    LoadRecentFilesPressed,
    RecentFilesLoaded(Result<Vec<String>, String>),
    RandomNotePressed,
    RandomNoteResolved(Result<String, String>),
    DailyNotePressed,
    DailyNoteResolved(Result<FileContent, String>),
    QuickSwitcherQueryChanged(String),
    QuickSwitcherOpenFirstPressed,
    SearchQueryChanged(String),
    SearchPressed,
    SearchLoaded(Result<PagedSearchResult, String>),
    SearchNextPagePressed,
    SearchPrevPagePressed,
    OutlineRefreshPressed,
    OutlineLoaded(Result<obsidian_types::NoteOutlineResponse, String>),
    OutlineSectionSelected(usize, String),
    SuggestionsRefreshPressed,
    SuggestionsLoaded(Result<obsidian_types::OrganizationSuggestionsResponse, String>),
    SuggestionActionPressed(usize, bool),
    SuggestionApplied(Result<obsidian_types::ApplyOrganizationSuggestionResponse, String>),
    UndoLastMlPressed,
    UndoLastMlLoaded(Result<obsidian_types::UndoMlActionResponse, String>),
    BacklinksRefreshPressed,
    BacklinksLoaded(Result<Vec<String>, String>),
    LoadTagsPressed,
    TagsLoaded(Result<Vec<obsidian_client::TagEntry>, String>),
    ToggleBookmarkPressed,

    LoadTreePressed,
    TreeLoaded(Result<Vec<FileNode>, String>),
    TreeEntrySelected(String),
    QuickReopenPressed(String),
    TabSelected(String),
    TabClosed(String),
    NewFilePathChanged(String),
    CreateFilePressed,
    FileCreated(Result<FileContent, String>),
    NewFolderPathChanged(String),
    CreateFolderPressed,
    FolderCreated(Result<String, String>),
    RenameFromPathChanged(String),
    RenameToPathChanged(String),
    RenamePathPressed,
    PathRenamed(Result<String, String>),
    DeleteTargetPathChanged(String),
    ArmDeletePressed,
    DeletePathPressed,
    DeleteCanceled,
    PathDeleted(Result<String, String>),
    TemplatePathChanged(String),
    TemplateModeSelected(TemplateInsertMode),
    InsertTemplatePressed,
    TemplateLoaded(Result<String, String>),
    RecordRecentPath(String),
    RecentPathRecorded(Result<String, String>),

    NotePathChanged(String),
    FrontmatterChanged(String),
    ToolbarActionPressed(ToolbarAction),
    EditorModeSelected(EditorMode),
    RenderPreviewRequested,
    RenderPreviewLoaded(Result<String, String>),
    PreviewLinkClicked(String),
    LoadNotePressed,
    NoteLoaded(Result<FileContent, String>),

    EditorChanged(String),
    SaveNotePressed,
    SaveNoteForcePressed,
    ConflictReloadPressed,
    ConflictDismissed,
    NoteSaved(Result<String, String>),

    ConnectEventsPressed,
    DisconnectEventsPressed,
    EventsConnected(Result<SharedWsStream, String>),
    PollNextEvent(SharedWsStream),
    EventMessageReceived(Result<WsMessage, String>),
    RetryEventConnection,

    PluginManagerPressed,
    PluginsRefreshPressed,
    PluginsLoaded(Result<Vec<PluginItem>, String>),
    TogglePluginPressed(String, bool),
    PluginToggled(Result<(String, bool), String>),
    ImportExportPressed,
    ImportLocalPathChanged(String),
    ImportVaultPathChanged(String),
    ImportFilePressed,
    FileImported(Result<String, String>),
    ExportVaultPathChanged(String),
    ExportLocalPathChanged(String),
    ExportFilePressed,
    FileExported(Result<String, String>),
    MediaLoaded(Result<MediaLoadResult, String>),
    OpenMediaExternallyPressed,
    MediaExternalOpened(Result<String, String>),

    // ── deployment mode / vault management ──────────────────────────────
    DeploymentModeSelected(DesktopMode),
    LocalBaseUrlChanged(String),
    NewVaultNameChanged(String),
    CreateVaultPressed,
    VaultCreated(Result<Vault, String>),

    // ── split pane ──────────────────────────────────────────────────────
    ToggleSplitPane,
    /// Open a file in the secondary (split) pane.
    SplitPaneTabSelected(String),

    // ── auto-save ───────────────────────────────────────────────────────
    /// Fired after a 2-second debounce; only saves if the generation matches.
    AutoSaveTick(u64),

    // ── diagnostics / feature flags ──────────────────────────────────────
    DiagnosticsPanelToggled,
    FeatureFlagMlChanged(bool),
    FeatureFlagMediaChanged(bool),
    FeatureFlagSyncChanged(bool),
    CopyDiagnosticsPressed,
}

fn update(state: &mut DesktopApp, message: Message) -> Task<Message> {
    match message {
        Message::LoadLocalSessionPressed => Task::perform(
            async move { load_persisted_session() },
            Message::LocalSessionLoaded,
        ),
        Message::WindowEventOccurred(event) => handle_window_event(state, event),
        Message::LocalSessionLoaded(result) => {
            match result {
                Ok(Some(session)) => {
                    apply_restored_session(state, session);
                    state.status = "Loaded local session snapshot".to_string();
                }
                Ok(None) => {
                    state.status = "No local session snapshot found yet".to_string();
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::BaseUrlChanged(value) => {
            state.base_url = value;
            Task::none()
        }
        Message::UsernameChanged(value) => {
            state.username = value;
            Task::none()
        }
        Message::PasswordChanged(value) => {
            state.password = value;
            Task::none()
        }
        Message::LoginPressed => {
            let base_url = if state.base_url.trim().is_empty() {
                "http://127.0.0.1:8080".to_string()
            } else {
                state.base_url.trim().to_string()
            };
            let local_url = state.local_base_url.trim().to_string();
            let username = state.username.trim().to_string();
            let password = state.password.clone();
            let mode = state.deployment_mode;
            state.status = "Logging in…".to_string();

            Task::perform(
                async move {
                    let mut client = match mode {
                        DesktopMode::Cloud => ObsidianClient::for_cloud(base_url),
                        DesktopMode::Standalone => ObsidianClient::for_standalone(base_url),
                        DesktopMode::Hybrid => {
                            let local = if local_url.is_empty() {
                                base_url.clone()
                            } else {
                                local_url
                            };
                            ObsidianClient::for_hybrid(base_url, local)
                        }
                    };
                    client
                        .login(&username, &password)
                        .await
                        .map_err(|e| format!("Login failed: {e}"))?;
                    Ok(client)
                },
                Message::LoginFinished,
            )
        }
        Message::LoginFinished(result) => {
            match result {
                Ok(client) => {
                    info!("Login successful");
                    state
                        .diagnostics
                        .push_log("[auth] Login successful".to_string());
                    state.client = Some(client);
                    state.status = "Login successful".to_string();
                    return Task::batch([
                        Task::done(Message::LoadVaultsPressed),
                        Task::done(Message::PreferencesPressed),
                    ]);
                }
                Err(err) => {
                    warn!(error = %err, "Login failed");
                    state.diagnostics.errors_logged += 1;
                    state
                        .diagnostics
                        .push_log(format!("[auth] Login failed: {err}"));
                    state.status = err;
                }
            }
            Task::none()
        }

        Message::LoadVaultsPressed => {
            let Some(client) = state.client.clone() else {
                state.status = "Please login first".to_string();
                return Task::none();
            };
            state.status = "Loading vaults…".to_string();
            Task::perform(
                async move {
                    client
                        .list_vaults()
                        .await
                        .map_err(|e| format!("Failed to load vaults: {e}"))
                },
                Message::VaultsLoaded,
            )
        }
        Message::VaultsLoaded(result) => {
            match result {
                Ok(vaults) => {
                    state.vaults = vaults;
                    state.status = format!("Loaded {} vault(s)", state.vaults.len());
                    if state.selected_vault_id.is_none() {
                        state.selected_vault_id = state.vaults.first().map(|v| v.id.clone());
                    }

                    if let Some(selected_vault_id) = state.selected_vault_id.clone() {
                        return Task::done(Message::VaultSelected(selected_vault_id));
                    }
                }
                Err(err) => {
                    state.status = err;
                }
            }
            Task::none()
        }
        Message::VaultSelected(vault_id) => {
            info!(vault_id = %vault_id, "Vault selected");
            state
                .diagnostics
                .push_log(format!("[vault] selected {vault_id}"));
            state.selected_vault_id = Some(vault_id);
            state.recent_files.clear();
            state.quick_switcher_query.clear();
            state.search_query.clear();
            state.search_results.clear();
            state.search_total_count = 0;
            state.search_page = 1;
            state.suggestion_items.clear();
            state.suggestion_source_path.clear();
            state.last_ml_receipt_id = None;
            state.last_ml_action_summary.clear();
            state.selected_tree_path = None;
            state.active_tab_path = None;
            state.open_tabs.clear();
            state.rename_from_path.clear();
            state.rename_to_path.clear();
            state.delete_target_path.clear();
            state.delete_confirmation_armed = false;
            state.outgoing_links.clear();
            state.backlink_paths.clear();
            state.tag_entries.clear();
            state.bookmarks.clear();
            clear_loaded_note(state);
            let _ = persist_session_state(state);
            let mut tasks = vec![
                Task::done(Message::LoadTreePressed),
                Task::done(Message::LoadRecentFilesPressed),
                Task::done(Message::LoadTagsPressed),
            ];
            if state.event_sync_requested && !state.event_sync_connected {
                tasks.push(Task::done(Message::ConnectEventsPressed));
            }
            Task::batch(tasks)
        }
        Message::PreferencesPressed => run_load_preferences(state, true),
        Message::PreferencesClosed => {
            state.preferences_visible = false;
            Task::none()
        }
        Message::PreferencesLoaded(result) => {
            match result {
                Ok(prefs) => {
                    apply_preferences_to_state(state, &prefs);
                    state.preferences_visible = true;
                    state.status = "Preferences loaded".to_string();
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::PreferencesThemeChanged(value) => {
            state.preferences_theme = value;
            Task::none()
        }
        Message::PreferencesFontSizeChanged(value) => {
            state.preferences_font_size_input = value;
            Task::none()
        }
        Message::PreferencesWindowLayoutChanged(value) => {
            state.preferences_window_layout_input = value;
            Task::none()
        }
        Message::PreferencesEditorModeSelected(mode) => {
            state.preferences_editor_mode = mode;
            state.editor_mode = match state.preferences_editor_mode {
                PreferenceEditorMode::Raw => EditorMode::Raw,
                PreferenceEditorMode::FullyRendered => EditorMode::Preview,
                PreferenceEditorMode::SideBySide | PreferenceEditorMode::FormattedRaw => {
                    EditorMode::Formatted
                }
            };
            let _ = persist_session_state(state);
            Task::none()
        }
        Message::PreferencesSavePressed => run_save_preferences(state),
        Message::PreferencesSaved(result) => {
            match result {
                Ok(prefs) => {
                    apply_preferences_to_state(state, &prefs);
                    state.status = "Preferences saved".to_string();
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::PreferencesResetPressed => run_reset_preferences(state),
        Message::PreferencesResetLoaded(result) => {
            match result {
                Ok(prefs) => {
                    apply_preferences_to_state(state, &prefs);
                    state.status = "Preferences reset".to_string();
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::LoadRecentFilesPressed => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                return Task::none();
            };

            Task::perform(
                async move {
                    client
                        .get_recent_files(&vault_id)
                        .await
                        .map_err(|e| format!("Failed to load recent files: {e}"))
                },
                Message::RecentFilesLoaded,
            )
        }
        Message::RecentFilesLoaded(result) => {
            if let Ok(recent_files) = result {
                state.recent_files = recent_files;
            }
            Task::none()
        }
        Message::RandomNotePressed => run_random_note(state),
        Message::RandomNoteResolved(result) => {
            match result {
                Ok(path) => {
                    state.status = format!("Random note: {path}");
                    return Task::done(Message::QuickReopenPressed(path));
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::DailyNotePressed => run_daily_note(state),
        Message::DailyNoteResolved(result) => {
            match result {
                Ok(file) => {
                    let path = file.path.clone();
                    apply_file_to_state(state, file);
                    state.status = format!("Daily note ready: {path}");
                    let _ = persist_session_state(state);
                    return Task::batch([
                        Task::done(Message::LoadTreePressed),
                        Task::done(Message::RecordRecentPath(path)),
                        Task::done(Message::RenderPreviewRequested),
                        Task::done(Message::OutlineRefreshPressed),
                        Task::done(Message::SuggestionsRefreshPressed),
                        Task::done(Message::BacklinksRefreshPressed),
                    ]);
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::QuickSwitcherQueryChanged(value) => {
            state.quick_switcher_query = value;
            Task::none()
        }
        Message::QuickSwitcherOpenFirstPressed => {
            if let Some(path) = find_quick_switch_match(state) {
                Task::done(Message::QuickReopenPressed(path))
            } else {
                state.status = "No quick switch match found".to_string();
                Task::none()
            }
        }
        Message::SearchQueryChanged(value) => {
            state.search_query = value;
            Task::none()
        }
        Message::SearchPressed => run_search(state),
        Message::SearchLoaded(result) => {
            match result {
                Ok(page) => {
                    state.search_results = page.results;
                    state.search_total_count = page.total_count;
                    state.search_page = page.page;
                    state.search_page_size = page.page_size;
                    state.status = format!(
                        "Search found {} result(s), page {}",
                        state.search_total_count, state.search_page
                    );
                }
                Err(err) => {
                    state.status = err;
                }
            }
            Task::none()
        }
        Message::SearchNextPagePressed => {
            state.search_page = state.search_page.saturating_add(1);
            run_search(state)
        }
        Message::SearchPrevPagePressed => {
            if state.search_page > 1 {
                state.search_page -= 1;
                run_search(state)
            } else {
                Task::none()
            }
        }
        Message::OutlineRefreshPressed => run_outline(state),
        Message::OutlineLoaded(result) => {
            match result {
                Ok(outline) => {
                    state.outline_sections = outline.sections;
                    state.outline_summary = outline.summary;
                    state.status = format!(
                        "Outline loaded: {} section(s)",
                        state.outline_sections.len()
                    );
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::OutlineSectionSelected(line_number, title) => {
            state.status = format!("Outline jump target: line {line_number} — {title}");
            Task::none()
        }
        Message::SuggestionsRefreshPressed => run_suggestions(state),
        Message::SuggestionsLoaded(result) => {
            match result {
                Ok(suggestions) => {
                    state.diagnostics.ml_requests += 1;
                    debug!(
                        count = suggestions.suggestions.len(),
                        "ML suggestions loaded"
                    );
                    state.suggestion_source_path = suggestions.file_path;
                    state.suggestion_items = suggestions.suggestions;
                    state.status =
                        format!("Loaded {} ML suggestion(s)", state.suggestion_items.len());
                }
                Err(err) => {
                    warn!(error = %err, "ML suggestions failed");
                    state.diagnostics.errors_logged += 1;
                    state
                        .diagnostics
                        .push_log(format!("[ml] suggestions error: {err}"));
                    state.status = err;
                }
            }
            Task::none()
        }
        Message::SuggestionActionPressed(index, dry_run) => {
            let Some(suggestion) = state.suggestion_items.get(index).cloned() else {
                state.status = "Suggestion no longer available".to_string();
                return Task::none();
            };
            run_apply_suggestion(state, suggestion, dry_run)
        }
        Message::SuggestionApplied(result) => {
            match result {
                Ok(response) => {
                    state.last_ml_receipt_id = response.receipt_id.clone();
                    state.last_ml_action_summary = response
                        .changes
                        .iter()
                        .map(|c| c.description.clone())
                        .collect::<Vec<_>>()
                        .join("; ");

                    let mode = if response.dry_run {
                        "Dry run"
                    } else {
                        "Applied"
                    };
                    state.status = format!("{mode}: {} change(s) prepared", response.changes.len());

                    let mut tasks: Vec<Task<Message>> =
                        vec![Task::done(Message::SuggestionsRefreshPressed)];
                    if !response.dry_run {
                        tasks.push(Task::done(Message::LoadTreePressed));
                        tasks.push(Task::done(Message::LoadTagsPressed));
                        tasks.push(Task::done(Message::BacklinksRefreshPressed));
                        tasks.push(Task::done(Message::OutlineRefreshPressed));

                        if let Some(path) = response.updated_file_path {
                            state.note_path = path;
                        }

                        if !state.note_path.trim().is_empty() {
                            tasks.push(Task::done(Message::LoadNotePressed));
                        }
                    }

                    return Task::batch(tasks);
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::UndoLastMlPressed => run_undo_last_ml(state),
        Message::UndoLastMlLoaded(result) => {
            match result {
                Ok(response) => {
                    state.last_ml_action_summary = response.description;
                    state.last_ml_receipt_id = None;
                    state.status = "Last ML action undone; refreshing note state…".to_string();
                    return Task::batch([
                        Task::done(Message::LoadTreePressed),
                        Task::done(Message::LoadTagsPressed),
                        Task::done(Message::LoadNotePressed),
                        Task::done(Message::BacklinksRefreshPressed),
                        Task::done(Message::OutlineRefreshPressed),
                        Task::done(Message::SuggestionsRefreshPressed),
                    ]);
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::BacklinksRefreshPressed => run_backlinks(state),
        Message::BacklinksLoaded(result) => {
            match result {
                Ok(paths) => {
                    state.backlink_paths = paths;
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::LoadTagsPressed => run_tags(state),
        Message::TagsLoaded(result) => {
            match result {
                Ok(entries) => {
                    state.tag_entries = entries
                        .into_iter()
                        .map(|entry| state::TagPanelEntry {
                            tag: entry.tag,
                            count: entry.count,
                            files: entry.files,
                        })
                        .collect();
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::ToggleBookmarkPressed => {
            let note_path = state.note_path.trim().to_string();
            if note_path.is_empty() {
                state.status = "Load a note before bookmarking".to_string();
                return Task::none();
            }

            if let Some(index) = state.bookmarks.iter().position(|path| path == &note_path) {
                state.bookmarks.remove(index);
                state.status = format!("Removed bookmark: {note_path}");
            } else {
                state.bookmarks.insert(0, note_path.clone());
                state.bookmarks.sort();
                state.bookmarks.dedup();
                state.status = format!("Bookmarked: {note_path}");
            }
            Task::none()
        }

        Message::LoadTreePressed => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                state.status = "Select a vault first".to_string();
                return Task::none();
            };
            state.status = "Loading file tree…".to_string();
            Task::perform(
                async move {
                    client
                        .get_file_tree(&vault_id)
                        .await
                        .map_err(|e| format!("Failed to load tree: {e}"))
                },
                Message::TreeLoaded,
            )
        }
        Message::TreeLoaded(result) => {
            match result {
                Ok(tree) => {
                    state.tree_entries = flatten_tree(&tree);
                    state.status = format!("Loaded {} tree entries", state.tree_entries.len());
                    if state.session_restore_in_progress {
                        return restore_next_session_tab(state);
                    }
                }
                Err(err) => {
                    state.status = err;
                }
            }
            Task::none()
        }
        Message::TreeEntrySelected(path) => {
            state.selected_tree_path = Some(path.clone());
            sync_rename_source_from_selection(state);
            let Some(entry) = state
                .tree_entries
                .iter()
                .find(|entry| entry.path == path)
                .cloned()
            else {
                state.status = "Tree entry not found".to_string();
                return Task::none();
            };

            if entry.is_directory {
                state.status = format!("Selected folder: {}", entry.path);
                return Task::none();
            }

            if activate_existing_tab(state, &entry.path) {
                state.status = format!("Switched to open tab {}", entry.path);
                return Task::none();
            }

            state.note_path = entry.path.clone();
            let _ = persist_session_state(state);
            Task::done(Message::LoadNotePressed)
        }
        Message::QuickReopenPressed(path) => {
            state.selected_tree_path = Some(path.clone());
            sync_rename_source_from_selection(state);
            if activate_existing_tab(state, &path) {
                add_recent_file(state, &path);
                return Task::done(Message::RecordRecentPath(path));
            }

            state.note_path = path;
            let _ = persist_session_state(state);
            Task::done(Message::LoadNotePressed)
        }
        Message::NewFilePathChanged(value) => {
            state.new_file_path = value;
            Task::none()
        }
        Message::CreateFilePressed => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                state.status = "Login and select a vault first".to_string();
                return Task::none();
            };

            let path = state.new_file_path.trim().to_string();
            if path.is_empty() {
                state.status = "Enter a file path to create".to_string();
                return Task::none();
            }

            state.status = format!("Creating note {path}…");
            Task::perform(
                async move {
                    client
                        .create_file(
                            &vault_id,
                            &obsidian_types::CreateFileRequest {
                                path,
                                content: Some(String::new()),
                            },
                        )
                        .await
                        .map_err(|e| format!("Failed to create note: {e}"))
                },
                Message::FileCreated,
            )
        }
        Message::FileCreated(result) => {
            match result {
                Ok(file) => {
                    let created_path = file.path.clone();
                    apply_file_to_state(state, file);
                    state.new_file_path.clear();
                    state.status = format!("Created note {created_path}");
                    return Task::batch([
                        Task::done(Message::LoadTreePressed),
                        Task::done(Message::RecordRecentPath(created_path)),
                    ]);
                }
                Err(err) => state.status = err,
            }

            Task::none()
        }
        Message::NewFolderPathChanged(value) => {
            state.new_folder_path = value;
            Task::none()
        }
        Message::CreateFolderPressed => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                state.status = "Login and select a vault first".to_string();
                return Task::none();
            };

            let path = state.new_folder_path.trim().to_string();
            if path.is_empty() {
                state.status = "Enter a folder path to create".to_string();
                return Task::none();
            }

            state.status = format!("Creating folder {path}…");
            Task::perform(
                async move {
                    client
                        .create_directory(&vault_id, &path)
                        .await
                        .map_err(|e| format!("Failed to create folder: {e}"))?;
                    Ok(path)
                },
                Message::FolderCreated,
            )
        }
        Message::FolderCreated(result) => {
            match result {
                Ok(path) => {
                    state.new_folder_path.clear();
                    state.selected_tree_path = Some(path.clone());
                    sync_rename_source_from_selection(state);
                    state.status = format!("Created folder {path}");
                    return Task::done(Message::LoadTreePressed);
                }
                Err(err) => state.status = err,
            }

            Task::none()
        }
        Message::RenameFromPathChanged(value) => {
            state.rename_from_path = value;
            Task::none()
        }
        Message::RenameToPathChanged(value) => {
            state.rename_to_path = value;
            Task::none()
        }
        Message::RenamePathPressed => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                state.status = "Login and select a vault first".to_string();
                return Task::none();
            };

            let from = state.rename_from_path.trim().to_string();
            let to = state.rename_to_path.trim().to_string();

            if from.is_empty() || to.is_empty() {
                state.status = "Enter both source and destination paths".to_string();
                return Task::none();
            }

            if from == to {
                state.status = "Choose a different destination path".to_string();
                return Task::none();
            }

            state.status = format!("Renaming {from} → {to}…");
            Task::perform(
                async move {
                    client
                        .rename_file(&vault_id, &from, &to, "fail")
                        .await
                        .map(|response| response.new_path)
                        .map_err(|e| format!("Failed to rename path: {e}"))
                },
                Message::PathRenamed,
            )
        }
        Message::PathRenamed(result) => {
            match result {
                Ok(new_path) => {
                    let from_path = state.rename_from_path.clone();
                    apply_rename_to_state(state, &from_path, &new_path);
                    state.rename_to_path.clear();
                    state.delete_confirmation_armed = false;
                    state.status = format!("Renamed {from_path} → {new_path}");
                    return Task::done(Message::LoadTreePressed);
                }
                Err(err) => state.status = err,
            }

            Task::none()
        }
        Message::DeleteTargetPathChanged(value) => {
            state.delete_target_path = value;
            state.delete_confirmation_armed = false;
            Task::none()
        }
        Message::ArmDeletePressed => {
            sync_rename_source_from_selection(state);
            if state.delete_target_path.trim().is_empty() {
                state.status = "Select or enter a path to delete".to_string();
                return Task::none();
            }

            state.delete_confirmation_armed = true;
            state.status = format!(
                "Delete armed for {}. Press Confirm Delete to move it to trash.",
                state.delete_target_path
            );
            Task::none()
        }
        Message::DeleteCanceled => {
            state.delete_confirmation_armed = false;
            state.status = "Delete canceled".to_string();
            Task::none()
        }
        Message::DeletePathPressed => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                state.status = "Login and select a vault first".to_string();
                return Task::none();
            };

            let target = state.delete_target_path.trim().to_string();
            if target.is_empty() {
                state.status = "Enter a path to delete".to_string();
                return Task::none();
            }

            if !state.delete_confirmation_armed {
                state.status = "Arm delete first, then confirm".to_string();
                return Task::none();
            }

            state.status = format!("Deleting {target} (moving to trash)…");
            Task::perform(
                async move {
                    client
                        .delete_file(&vault_id, &target)
                        .await
                        .map_err(|e| format!("Failed to delete path: {e}"))?;
                    Ok(target)
                },
                Message::PathDeleted,
            )
        }
        Message::PathDeleted(result) => {
            match result {
                Ok(target) => {
                    apply_delete_to_state(state, &target);
                    state.rename_from_path = state.note_path.clone();
                    state.status = format!("Deleted {target} to trash");
                    return Task::done(Message::LoadTreePressed);
                }
                Err(err) => {
                    state.delete_confirmation_armed = false;
                    state.status = err;
                }
            }

            Task::none()
        }
        Message::TemplatePathChanged(value) => {
            state.template_path = value;
            Task::none()
        }
        Message::TemplateModeSelected(mode) => {
            state.template_insert_mode = mode;
            Task::none()
        }
        Message::InsertTemplatePressed => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                state.status = "Login and select a vault first".to_string();
                return Task::none();
            };

            let template_path = state.template_path.trim().to_string();
            if template_path.is_empty() {
                state.status = "Enter a template path (e.g. Templates/Daily Note.md)".to_string();
                return Task::none();
            }

            state.status = format!("Loading template {template_path}…");
            Task::perform(
                async move {
                    client
                        .read_file(&vault_id, &template_path)
                        .await
                        .map(|file| file.content)
                        .map_err(|e| format!("Failed to load template: {e}"))
                },
                Message::TemplateLoaded,
            )
        }
        Message::TemplateLoaded(result) => {
            match result {
                Ok(template_content) => {
                    let merged = process_template_content(
                        &template_content,
                        &state.note_path,
                        &state.note_content,
                        state.template_insert_mode,
                    );
                    state.note_content = merged.clone();
                    state.preview_content = merged;
                    refresh_preview_markdown(state);
                    state.note_is_dirty = true;
                    state.conflict_active = false;
                    state.conflict_message.clear();
                    sync_active_tab_from_editor(state);
                    state.status =
                        "Template inserted into editor. Save to persist changes.".to_string();

                    if matches!(
                        state.editor_mode,
                        EditorMode::Formatted | EditorMode::Preview
                    ) {
                        return Task::done(Message::RenderPreviewRequested);
                    }
                }
                Err(err) => state.status = err,
            }

            Task::none()
        }
        Message::TabSelected(path) => {
            state.selected_tree_path = Some(path.clone());
            sync_rename_source_from_selection(state);
            if activate_existing_tab(state, &path) {
                add_recent_file(state, &path);
                state.status = format!("Active tab: {path}");
                let _ = persist_session_state(state);
                return Task::done(Message::RecordRecentPath(path));
            }

            state.status = format!("Tab not found: {path}");
            Task::none()
        }
        Message::TabClosed(path) => {
            let was_active = state.active_tab_path.as_deref() == Some(path.as_str());
            state.open_tabs.retain(|tab| tab.path != path);

            if state.open_tabs.is_empty() {
                state.active_tab_path = None;
                state.selected_tree_path = None;
                clear_loaded_note(state);
                state.status = "Closed final tab".to_string();
                let _ = persist_session_state(state);
                return Task::none();
            }

            if was_active {
                let fallback_path = state.open_tabs.last().map(|tab| tab.path.clone());
                state.active_tab_path = fallback_path.clone();
                if let Some(next_path) = fallback_path {
                    state.selected_tree_path = Some(next_path.clone());
                    let _ = activate_existing_tab(state, &next_path);
                    state.delete_target_path = next_path.clone();
                    state.status = format!("Closed tab and switched to {next_path}");
                }
            } else {
                state.status = format!("Closed tab {path}");
            }

            let _ = persist_session_state(state);
            Task::none()
        }
        Message::RecordRecentPath(path) => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                return Task::none();
            };

            add_recent_file(state, &path);

            Task::perform(
                async move {
                    client
                        .record_recent_file(&vault_id, &path)
                        .await
                        .map_err(|e| format!("Failed to record recent file: {e}"))?;
                    Ok(path)
                },
                Message::RecentPathRecorded,
            )
        }
        Message::RecentPathRecorded(_result) => Task::none(),

        Message::NotePathChanged(value) => {
            state.note_path = value;
            Task::none()
        }
        Message::FrontmatterChanged(value) => {
            state.note_frontmatter_raw = value;
            match parse_frontmatter(&state.note_frontmatter_raw) {
                Ok(frontmatter) => {
                    state.note_frontmatter_summary =
                        summarize_frontmatter_value(frontmatter.as_ref());
                    state.note_is_dirty = true;
                    sync_active_tab_from_editor(state);
                }
                Err(err) => {
                    state.note_frontmatter_summary = err;
                    state.note_is_dirty = true;
                    sync_active_tab_from_editor(state);
                }
            }
            Task::none()
        }
        Message::ToolbarActionPressed(action) => {
            state.note_content = apply_toolbar_action(&state.note_content, action);
            state.preview_content = state.note_content.clone();
            refresh_preview_markdown(state);
            state.note_is_dirty = true;
            sync_active_tab_from_editor(state);
            if matches!(
                state.editor_mode,
                EditorMode::Formatted | EditorMode::Preview
            ) {
                Task::done(Message::RenderPreviewRequested)
            } else {
                Task::none()
            }
        }
        Message::EditorModeSelected(mode) => {
            state.editor_mode = mode;
            let _ = persist_session_state(state);
            if matches!(
                state.editor_mode,
                EditorMode::Formatted | EditorMode::Preview
            ) {
                Task::done(Message::RenderPreviewRequested)
            } else {
                Task::none()
            }
        }
        Message::RenderPreviewRequested => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                return Task::none();
            };

            let content = state.note_content.clone();
            let current_file = if state.note_path.trim().is_empty() {
                None
            } else {
                Some(state.note_path.clone())
            };

            Task::perform(
                async move {
                    client
                        .render_markdown_in_vault(&vault_id, &content, current_file.as_deref())
                        .await
                        .map_err(|e| format!("Failed to render preview: {e}"))
                },
                Message::RenderPreviewLoaded,
            )
        }
        Message::RenderPreviewLoaded(result) => {
            match result {
                Ok(rendered_html) => {
                    state.rendered_preview_html = rendered_html;
                    state.preview_render_error = None;
                }
                Err(err) => {
                    state.preview_render_error = Some(err);
                    state.rendered_preview_html.clear();
                }
            }
            Task::none()
        }
        Message::PreviewLinkClicked(url) => {
            state.status = format!("Preview link clicked: {url}");
            Task::none()
        }
        Message::LoadNotePressed => {
            let (Some(client), Some(vault_id)) =
                (state.client.clone(), state.selected_vault_id.clone())
            else {
                state.status = "Login and select a vault first".to_string();
                return Task::none();
            };

            let note_path = state.note_path.trim().to_string();
            if note_path.is_empty() {
                state.status = "Enter a note path to load".to_string();
                return Task::none();
            }

            let file_kind = file_kind_from_path(&note_path);
            if is_media_file_kind(file_kind) {
                let _ = (client, vault_id);
                return run_load_media(state);
            }

            state.status = format!("Loading note {note_path}…");
            Task::perform(
                async move {
                    client
                        .read_file(&vault_id, &note_path)
                        .await
                        .map_err(|e| format!("Failed to read note: {e}"))
                },
                Message::NoteLoaded,
            )
        }
        Message::NoteLoaded(result) => {
            match result {
                Ok(file) => {
                    let path = file.path.clone();
                    info!(path = %path, "Note loaded");
                    state.diagnostics.notes_loaded += 1;
                    state.diagnostics.push_log(format!("[note] loaded {path}"));
                    apply_file_to_state(state, file);
                    refresh_outgoing_links(state);
                    state.status = "Note loaded".to_string();
                    let _ = persist_session_state(state);

                    let mut tasks = vec![
                        Task::done(Message::RecordRecentPath(path)),
                        Task::done(Message::RenderPreviewRequested),
                        Task::done(Message::OutlineRefreshPressed),
                        Task::done(Message::SuggestionsRefreshPressed),
                        Task::done(Message::BacklinksRefreshPressed),
                    ];

                    if state.session_restore_in_progress {
                        tasks.push(restore_next_session_tab(state));
                    }

                    return Task::batch(tasks);
                }
                Err(err) => {
                    warn!(error = %err, "Note load failed");
                    state.diagnostics.errors_logged += 1;
                    state
                        .diagnostics
                        .push_log(format!("[note] load error: {err}"));
                    state.status = err;
                }
            }
            Task::none()
        }

        Message::EditorChanged(content) => {
            state.note_content = content.clone();
            state.preview_content = content;
            refresh_preview_markdown(state);
            refresh_outgoing_links(state);
            state.note_is_dirty = true;
            state.conflict_active = false;
            state.conflict_message.clear();
            sync_active_tab_from_editor(state);

            // Bump auto-save generation and schedule a debounced save.
            state.auto_save_generation = state.auto_save_generation.wrapping_add(1);
            let generation = state.auto_save_generation;
            let auto_save_task = Task::perform(
                async move {
                    sleep(Duration::from_secs(2)).await;
                    generation
                },
                Message::AutoSaveTick,
            );

            if matches!(
                state.editor_mode,
                EditorMode::Formatted | EditorMode::Preview
            ) {
                Task::batch([
                    Task::done(Message::RenderPreviewRequested),
                    Task::done(Message::OutlineRefreshPressed),
                    auto_save_task,
                ])
            } else {
                Task::batch([
                    Task::done(Message::OutlineRefreshPressed),
                    auto_save_task,
                ])
            }
        }
        Message::SaveNotePressed => save_note_task(state, false),
        Message::SaveNoteForcePressed => save_note_task(state, true),
        Message::ConflictReloadPressed => {
            state.conflict_active = false;
            state.conflict_message.clear();
            Task::done(Message::LoadNotePressed)
        }
        Message::ConflictDismissed => {
            state.conflict_active = false;
            state.conflict_message.clear();
            state.status = "Conflict notice dismissed; local edits kept unsaved".to_string();
            Task::none()
        }
        Message::NoteSaved(result) => {
            match result {
                Ok(modified) => {
                    info!(path = %state.note_path, "Note saved");
                    state.diagnostics.notes_saved += 1;
                    state
                        .diagnostics
                        .push_log(format!("[note] saved {} at {modified}", state.note_path));
                    state.note_modified = Some(modified.clone());
                    state.note_is_dirty = false;
                    state.conflict_active = false;
                    state.conflict_message.clear();
                    sync_active_tab_from_editor(state);
                    state.status = format!("Note saved at {modified}");
                }
                Err(err) => {
                    if is_conflict_error(&err) {
                        warn!(path = %state.note_path, "Save conflict detected");
                        state
                            .diagnostics
                            .push_log(format!("[note] save conflict: {err}"));
                        state.conflict_active = true;
                        state.conflict_message = err.clone();
                        state.status =
                            "Save conflict detected. Review choices in the conflict panel."
                                .to_string();
                    } else {
                        error!(error = %err, "Note save failed");
                        state.diagnostics.errors_logged += 1;
                        state
                            .diagnostics
                            .push_log(format!("[note] save error: {err}"));
                        state.status = err;
                    }
                }
            }
            Task::none()
        }

        Message::ConnectEventsPressed => {
            let Some(client) = state.client.clone() else {
                state.status = "Please login first".to_string();
                return Task::none();
            };

            if state.event_sync_connected {
                state.status = "Event sync loop already connected".to_string();
                return Task::none();
            }

            state.event_sync_requested = true;
            state.event_sync_last_message = if state.event_sync_retry_attempt > 0 {
                format!(
                    "Reconnecting… attempt {}",
                    state.event_sync_retry_attempt.saturating_add(1)
                )
            } else {
                "Connecting…".to_string()
            };
            state.status = "Connecting event sync loop…".to_string();
            Task::perform(
                async move {
                    let stream = client
                        .connect_ws()
                        .await
                        .map_err(|e| format!("WebSocket connect failed: {e}"))?;
                    Ok(SharedWsStream(Arc::new(iced::futures::lock::Mutex::new(
                        stream,
                    ))))
                },
                Message::EventsConnected,
            )
        }
        Message::DisconnectEventsPressed => {
            state.event_sync_requested = false;
            state.event_sync_connected = false;
            state.event_sync_retry_attempt = 0;
            state.event_sync_stream = None;
            state.event_sync_last_message = "Disconnected".to_string();
            state.status = "Event sync loop disconnected".to_string();
            Task::none()
        }
        Message::EventsConnected(result) => {
            match result {
                Ok(stream) => {
                    info!("Event sync loop connected");
                    state.diagnostics.push_log("[sync] connected".to_string());
                    state.event_sync_connected = true;
                    state.event_sync_retry_attempt = 0;
                    state.event_sync_last_message = "Connected".to_string();
                    state.event_sync_stream = Some(stream.clone());
                    state.status = "Event sync loop connected".to_string();
                    return Task::done(Message::PollNextEvent(stream));
                }
                Err(err) => {
                    warn!(error = %err, "Event sync connection failed");
                    state.diagnostics.errors_logged += 1;
                    state
                        .diagnostics
                        .push_log(format!("[sync] connect error: {err}"));
                    state.event_sync_connected = false;
                    state.event_sync_stream = None;
                    state.event_sync_last_message = err.clone();
                    state.status = err;
                    if state.event_sync_requested {
                        return schedule_event_reconnect(state);
                    }
                }
            }
            Task::none()
        }
        Message::PollNextEvent(stream) => {
            if !state.event_sync_requested {
                return Task::none();
            }

            state.event_sync_stream = Some(stream.clone());
            Task::perform(
                read_next_event_message(stream),
                Message::EventMessageReceived,
            )
        }
        Message::EventMessageReceived(result) => match result {
            Ok(message) => {
                state.diagnostics.sync_messages_received += 1;
                debug!(?message, "WS event received");
                handle_event_message(state, message)
            }
            Err(err) => {
                warn!(error = %err, "Event sync disconnected");
                state.diagnostics.errors_logged += 1;
                state
                    .diagnostics
                    .push_log(format!("[sync] disconnected: {err}"));
                state.event_sync_connected = false;
                state.event_sync_stream = None;
                state.event_sync_last_message = format!("Disconnected: {err}");
                state.status = format!("Event sync disconnected: {err}");
                if state.event_sync_requested {
                    schedule_event_reconnect(state)
                } else {
                    Task::none()
                }
            }
        },
        Message::RetryEventConnection => {
            if state.event_sync_requested && !state.event_sync_connected {
                Task::done(Message::ConnectEventsPressed)
            } else {
                Task::none()
            }
        }

        Message::PluginManagerPressed => {
            state.plugin_panel_visible = !state.plugin_panel_visible;
            if state.plugin_panel_visible {
                run_load_plugins(state)
            } else {
                Task::none()
            }
        }
        Message::PluginsRefreshPressed => run_load_plugins(state),
        Message::PluginsLoaded(result) => {
            match result {
                Ok(items) => {
                    state.plugin_status = format!("{} plugin(s) found", items.len());
                    state.plugins = items;
                }
                Err(err) => state.plugin_status = format!("Failed to load plugins: {err}"),
            }
            Task::none()
        }
        Message::TogglePluginPressed(plugin_id, enabled) => {
            let Some(client) = state.client.clone() else {
                state.status = "Please login first".to_string();
                return Task::none();
            };
            let pid = plugin_id.clone();
            state.plugin_status = format!(
                "{} {}…",
                if enabled { "Enabling" } else { "Disabling" },
                plugin_id
            );
            Task::perform(
                async move {
                    client
                        .toggle_plugin(&pid, enabled)
                        .await
                        .map(|_| (pid, enabled))
                        .map_err(|e| e.to_string())
                },
                Message::PluginToggled,
            )
        }
        Message::PluginToggled(result) => {
            match result {
                Ok((plugin_id, enabled)) => {
                    if let Some(p) = state.plugins.iter_mut().find(|p| p.id == plugin_id) {
                        p.enabled = enabled;
                        p.state_label = if enabled {
                            "loaded".to_string()
                        } else {
                            "disabled".to_string()
                        };
                    }
                    state.plugin_status = format!(
                        "Plugin {} {}",
                        plugin_id,
                        if enabled { "enabled" } else { "disabled" }
                    );
                }
                Err(err) => state.plugin_status = format!("Toggle failed: {err}"),
            }
            Task::none()
        }
        Message::ImportExportPressed => {
            state.import_export_visible = !state.import_export_visible;
            Task::none()
        }
        Message::ImportLocalPathChanged(v) => {
            state.import_local_path = v;
            Task::none()
        }
        Message::ImportVaultPathChanged(v) => {
            state.import_vault_path = v;
            Task::none()
        }
        Message::ImportFilePressed => run_import_file(state),
        Message::FileImported(result) => {
            match result {
                Ok(msg) => state.import_status = msg,
                Err(err) => state.import_status = format!("Import failed: {err}"),
            }
            Task::none()
        }
        Message::ExportVaultPathChanged(v) => {
            state.export_vault_path = v;
            Task::none()
        }
        Message::ExportLocalPathChanged(v) => {
            state.export_local_path = v;
            Task::none()
        }
        Message::ExportFilePressed => run_export_file(state),
        Message::FileExported(result) => {
            match result {
                Ok(msg) => state.export_status = msg,
                Err(err) => state.export_status = format!("Export failed: {err}"),
            }
            Task::none()
        }
        Message::MediaLoaded(result) => {
            match result {
                Ok((path, file_kind, source_url, image_bytes)) => {
                    let media_image = image_bytes.map(iced::widget::image::Handle::from_bytes);
                    apply_media_to_state(state, path.clone(), file_kind, source_url, media_image);
                    state.status = format!("Loaded {} {}", file_kind_label(file_kind), path);
                    let _ = persist_session_state(state);
                }
                Err(err) => state.status = err,
            }
            Task::none()
        }
        Message::OpenMediaExternallyPressed => run_open_media_externally(state),
        Message::MediaExternalOpened(result) => {
            match result {
                Ok(message) => {
                    state.media_status = message.clone();
                    state.status = message;
                }
                Err(err) => {
                    state.media_status = format!("Open failed: {err}");
                    state.status = state.media_status.clone();
                }
            }
            Task::none()
        }

        // ── split pane ─────────────────────────────────────────────────────
        Message::ToggleSplitPane => {
            state.split_pane_enabled = !state.split_pane_enabled;
            if !state.split_pane_enabled {
                state.split_pane_active_tab = None;
            }
            state.status = if state.split_pane_enabled {
                "Split pane enabled — select a tab for the right pane".to_string()
            } else {
                "Split pane disabled".to_string()
            };
            Task::none()
        }
        Message::SplitPaneTabSelected(path) => {
            state.split_pane_active_tab = Some(path.clone());
            state.status = format!("Right pane: {path}");
            Task::none()
        }

        // ── auto-save ──────────────────────────────────────────────────────
        Message::AutoSaveTick(generation) => {
            // Only auto-save if the generation still matches (no new edits since
            // the timer was scheduled), the note is dirty, and we have an active path.
            if generation == state.auto_save_generation
                && state.note_is_dirty
                && !state.note_path.trim().is_empty()
                && state.client.is_some()
                && state.selected_vault_id.is_some()
            {
                debug!(path = %state.note_path, "Auto-save triggered");
                state
                    .diagnostics
                    .push_log(format!("[auto-save] saving {}", state.note_path));
                return save_note_task(state, false);
            }
            Task::none()
        }

        // ── deployment mode / vault management ──────────────────────────────
        Message::DeploymentModeSelected(mode) => {
            state.deployment_mode = mode;
            info!(mode = ?mode, "Deployment mode changed");
            Task::none()
        }
        Message::LocalBaseUrlChanged(value) => {
            state.local_base_url = value;
            Task::none()
        }
        Message::NewVaultNameChanged(value) => {
            state.new_vault_name = value;
            Task::none()
        }
        Message::CreateVaultPressed => {
            let Some(client) = state.client.clone() else {
                state.status = "Login first to create a vault".to_string();
                return Task::none();
            };
            let name = state.new_vault_name.trim().to_string();
            if name.is_empty() {
                state.status = "Enter a vault name to create".to_string();
                return Task::none();
            }
            state.status = format!("Creating vault '{name}'…");
            Task::perform(
                async move {
                    client
                        .create_vault(&CreateVaultRequest { name, path: None })
                        .await
                        .map_err(|e| format!("Failed to create vault: {e}"))
                },
                Message::VaultCreated,
            )
        }
        Message::VaultCreated(result) => {
            match result {
                Ok(vault) => {
                    state.new_vault_name.clear();
                    info!(vault_name = %vault.name, "Vault created");
                    state.status = format!("Created vault '{}'", vault.name);
                    return Task::done(Message::LoadVaultsPressed);
                }
                Err(err) => {
                    warn!(error = %err, "Failed to create vault");
                    state.status = err;
                }
            }
            Task::none()
        }

        // ── diagnostics / feature flags ──────────────────────────────────
        Message::DiagnosticsPanelToggled => {
            state.feature_flags.diagnostics_panel = !state.feature_flags.diagnostics_panel;
            info!(
                diagnostics_panel = state.feature_flags.diagnostics_panel,
                "Diagnostics panel toggled"
            );
            Task::none()
        }
        Message::FeatureFlagMlChanged(enabled) => {
            state.feature_flags.ml_features = enabled;
            info!(ml_features = enabled, "Feature flag: ml_features changed");
            state
                .diagnostics
                .push_log(format!("[flag] ml_features = {enabled}"));
            Task::none()
        }
        Message::FeatureFlagMediaChanged(enabled) => {
            state.feature_flags.media_preview = enabled;
            info!(
                media_preview = enabled,
                "Feature flag: media_preview changed"
            );
            state
                .diagnostics
                .push_log(format!("[flag] media_preview = {enabled}"));
            Task::none()
        }
        Message::FeatureFlagSyncChanged(enabled) => {
            state.feature_flags.event_sync = enabled;
            info!(event_sync = enabled, "Feature flag: event_sync changed");
            state
                .diagnostics
                .push_log(format!("[flag] event_sync = {enabled}"));
            Task::none()
        }
        Message::CopyDiagnosticsPressed => {
            let report = state.diagnostics.as_report(&state.feature_flags);
            info!(bytes = report.len(), "Diagnostics report → clipboard");
            state.status = "Diagnostics report copied to clipboard".to_string();
            iced::clipboard::write(report)
        }
    }
}

fn subscription(_state: &DesktopApp) -> Subscription<Message> {
    event::listen().map(Message::WindowEventOccurred)
}

fn restore_next_session_tab(state: &mut DesktopApp) -> Task<Message> {
    if let Some(next_path) = state.pending_session_tab_paths.first().cloned() {
        state.pending_session_tab_paths.remove(0);
        state.session_restore_in_progress = !state.pending_session_tab_paths.is_empty();
        state.note_path = next_path.clone();
        state.selected_tree_path = Some(next_path);
        let _ = persist_session_state(state);
        return Task::done(Message::LoadNotePressed);
    }

    state.session_restore_in_progress = false;
    Task::none()
}

fn handle_window_event(state: &mut DesktopApp, event: Event) -> Task<Message> {
    let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
        return Task::none();
    };

    let command = modifiers.control() || modifiers.logo();
    if !command {
        return Task::none();
    }

    match key.as_ref() {
        keyboard::Key::Character("s") | keyboard::Key::Character("S") => {
            state.status = if modifiers.shift() {
                "Shortcut: force save".to_string()
            } else {
                "Shortcut: save".to_string()
            };
            if modifiers.shift() {
                Task::done(Message::SaveNoteForcePressed)
            } else {
                Task::done(Message::SaveNotePressed)
            }
        }
        keyboard::Key::Character("f") | keyboard::Key::Character("F") => {
            state.status = "Shortcut: search".to_string();
            Task::done(Message::SearchPressed)
        }
        keyboard::Key::Character("k")
        | keyboard::Key::Character("K")
        | keyboard::Key::Character("p")
        | keyboard::Key::Character("P") => {
            state.status = "Shortcut: quick switch".to_string();
            Task::done(Message::QuickSwitcherOpenFirstPressed)
        }
        keyboard::Key::Character("1") => Task::done(Message::EditorModeSelected(EditorMode::Raw)),
        keyboard::Key::Character("2") => {
            Task::done(Message::EditorModeSelected(EditorMode::Formatted))
        }
        keyboard::Key::Character("3") => {
            Task::done(Message::EditorModeSelected(EditorMode::Preview))
        }
        keyboard::Key::Character("e") | keyboard::Key::Character("E") => {
            if state.event_sync_requested {
                state.status = "Shortcut: stop sync".to_string();
                Task::done(Message::DisconnectEventsPressed)
            } else {
                state.status = "Shortcut: connect sync".to_string();
                Task::done(Message::ConnectEventsPressed)
            }
        }
        keyboard::Key::Character("\\") => {
            state.status = "Shortcut: toggle split pane".to_string();
            Task::done(Message::ToggleSplitPane)
        }
        keyboard::Key::Character("r") | keyboard::Key::Character("R") => {
            state.status = "Shortcut: refresh active note".to_string();
            Task::batch([
                Task::done(Message::LoadNotePressed),
                Task::done(Message::LoadTreePressed),
            ])
        }
        _ => Task::none(),
    }
}

async fn read_next_event_message(stream: SharedWsStream) -> Result<WsMessage, String> {
    let mut stream = stream.0.lock().await;
    let Some(frame) = stream.next().await else {
        return Err("server closed the event stream".to_string());
    };

    match frame.map_err(|err| err.to_string())? {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            serde_json::from_str::<WsMessage>(&text)
                .map_err(|err| format!("Invalid event payload: {err}"))
        }
        tokio_tungstenite::tungstenite::Message::Close(frame) => Err(frame
            .map(|frame| frame.reason.to_string())
            .filter(|reason| !reason.is_empty())
            .unwrap_or_else(|| "server requested close".to_string())),
        tokio_tungstenite::tungstenite::Message::Ping(_) => Ok(WsMessage::SyncPing),
        tokio_tungstenite::tungstenite::Message::Pong(_) => Ok(WsMessage::SyncPong {
            server_time: Utc::now().timestamp_millis(),
        }),
        _ => Err("unsupported websocket frame".to_string()),
    }
}

fn handle_event_message(state: &mut DesktopApp, message: WsMessage) -> Task<Message> {
    let Some(stream) = state.event_sync_stream.clone() else {
        return Task::none();
    };

    let mut tasks = vec![Task::done(Message::PollNextEvent(stream))];

    match message {
        WsMessage::FileChanged {
            vault_id,
            path,
            event_type,
            ..
        } => {
            state.event_sync_connected = true;

            if let FileChangeType::Renamed { from, to } = &event_type {
                apply_rename_to_state(state, from, to);
                let _ = persist_session_state(state);
            } else if matches!(event_type, FileChangeType::Deleted) {
                apply_delete_to_state(state, &path);
                let _ = persist_session_state(state);
            }

            state.event_sync_last_message = format!("{}: {}", describe_change(&event_type), path);

            if state.selected_vault_id.as_deref() == Some(vault_id.as_str()) {
                tasks.push(Task::done(Message::LoadTreePressed));
                tasks.push(Task::done(Message::LoadTagsPressed));

                let current_path = state.note_path.trim().to_string();
                let current_matches = match &event_type {
                    FileChangeType::Renamed { from, to } => {
                        current_path == *from || current_path == *to
                    }
                    _ => current_path == path,
                };

                if current_matches {
                    if state.note_is_dirty {
                        state.status = format!(
                            "Remote change detected for {} while local edits are unsaved",
                            current_path
                        );
                    } else if !matches!(event_type, FileChangeType::Deleted) {
                        tasks.push(Task::done(Message::LoadNotePressed));
                        tasks.push(Task::done(Message::BacklinksRefreshPressed));
                        tasks.push(Task::done(Message::OutlineRefreshPressed));
                        tasks.push(Task::done(Message::SuggestionsRefreshPressed));
                        state.status = format!(
                            "Remote {} detected for {} — refreshing",
                            describe_change(&event_type),
                            path
                        );
                    } else {
                        state.status = format!("Remote delete detected for {}", path);
                    }
                } else {
                    state.status = format!(
                        "Remote {} detected for {}",
                        describe_change(&event_type),
                        path
                    );
                }
            }
        }
        WsMessage::SyncPing => {
            state.event_sync_last_message = "Ping received".to_string();
        }
        WsMessage::SyncPong { server_time } => {
            state.event_sync_last_message = format!("Pong @ {server_time}");
        }
        WsMessage::Error { message } => {
            state.event_sync_last_message = format!("Server error: {message}");
            state.status = format!("Event sync server error: {message}");
        }
    }

    Task::batch(tasks)
}

fn schedule_event_reconnect(state: &mut DesktopApp) -> Task<Message> {
    state.event_sync_retry_attempt = state.event_sync_retry_attempt.saturating_add(1);
    let exponent: u32 = state.event_sync_retry_attempt.min(3);
    let delay_secs = 2u64.saturating_pow(exponent).min(8);
    state.event_sync_last_message = format!(
        "Reconnect scheduled in {}s (attempt {})",
        delay_secs, state.event_sync_retry_attempt
    );

    Task::perform(
        async move {
            sleep(Duration::from_secs(delay_secs)).await;
        },
        |_| Message::RetryEventConnection,
    )
}

fn describe_change(change: &FileChangeType) -> &'static str {
    match change {
        FileChangeType::Created => "create",
        FileChangeType::Modified => "update",
        FileChangeType::Deleted => "delete",
        FileChangeType::Renamed { .. } => "rename",
    }
}

fn save_note_task(state: &mut DesktopApp, force: bool) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        state.status = "Login and select a vault first".to_string();
        return Task::none();
    };

    let note_path = state.note_path.trim().to_string();
    let content = state.note_content.clone();
    let frontmatter = match parse_frontmatter(&state.note_frontmatter_raw) {
        Ok(frontmatter) => frontmatter,
        Err(err) => {
            state.status = err;
            return Task::none();
        }
    };
    if note_path.is_empty() {
        state.status = "Enter a note path to save".to_string();
        return Task::none();
    }

    let last_modified = if force {
        None
    } else {
        state.note_modified.as_deref().and_then(parse_rfc3339_utc)
    };

    if force {
        state.status = format!("Force-saving {note_path} (overwrite latest disk version)…");
    } else {
        state.status = format!("Saving {note_path} with conflict check…");
    }
    Task::perform(
        async move {
            let request = obsidian_types::UpdateFileRequest {
                content,
                last_modified,
                frontmatter,
            };
            let saved = client
                .write_file(&vault_id, &note_path, &request)
                .await
                .map_err(|e| format!("Failed to save note: {e}"))?;
            Ok(saved.modified.to_rfc3339())
        },
        Message::NoteSaved,
    )
}

fn run_load_plugins(state: &mut DesktopApp) -> Task<Message> {
    let Some(client) = state.client.clone() else {
        state.plugin_status = "Please login first".to_string();
        return Task::none();
    };
    state.plugin_status = "Loading plugins…".to_string();
    Task::perform(
        async move {
            client
                .list_plugins()
                .await
                .map(|resp| parse_plugin_items(&resp.plugins))
                .map_err(|e| format!("Failed to load plugins: {e}"))
        },
        Message::PluginsLoaded,
    )
}

fn run_random_note(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        state.status = "Login and select a vault first".to_string();
        return Task::none();
    };

    state.status = "Finding a random note…".to_string();
    Task::perform(
        async move {
            client
                .get_random_note(&vault_id)
                .await
                .map(|response| response.path)
                .map_err(|_| "No markdown files found in this vault".to_string())
        },
        Message::RandomNoteResolved,
    )
}

fn run_daily_note(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        state.status = "Login and select a vault first".to_string();
        return Task::none();
    };

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    state.status = format!("Opening daily note for {today}…");
    Task::perform(
        async move {
            client
                .get_daily_note(&vault_id, &today)
                .await
                .map_err(|e| format!("Failed to get daily note: {e}"))
        },
        Message::DailyNoteResolved,
    )
}

fn run_load_media(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        state.status = "Login and select a vault first".to_string();
        return Task::none();
    };

    let note_path = state.note_path.trim().to_string();
    if note_path.is_empty() {
        state.status = "Enter a file path to load".to_string();
        return Task::none();
    }

    let file_kind = file_kind_from_path(&note_path);
    state.status = format!("Loading {} {}…", file_kind_label(file_kind), note_path);

    Task::perform(
        async move {
            let source_url = client.raw_file_url(&vault_id, &note_path);
            let image_bytes = if matches!(file_kind, FileKind::Image) {
                Some(
                    client
                        .download_file_bytes(&vault_id, &note_path)
                        .await
                        .map_err(|e| format!("Failed to load image preview: {e}"))?,
                )
            } else {
                None
            };

            Ok((note_path, file_kind, source_url, image_bytes))
        },
        Message::MediaLoaded,
    )
}

fn run_open_media_externally(state: &mut DesktopApp) -> Task<Message> {
    let target = if !state.media_source_url.trim().is_empty() {
        state.media_source_url.trim().to_string()
    } else {
        state.note_path.trim().to_string()
    };

    if target.is_empty() {
        state.media_status = "Load a media file first".to_string();
        return Task::none();
    }

    state.media_status = format!("Opening {} externally…", state.note_path);

    Task::perform(
        async move {
            open_external_target(&target)?;
            Ok(format!("Opened externally: {target}"))
        },
        Message::MediaExternalOpened,
    )
}

fn open_external_target(target: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .map_err(|e| format!("Failed to open target: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open target: {e}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open target: {e}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Opening external targets is not supported on this platform".to_string())
}

fn run_import_file(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        state.import_status = "Login and select a vault first".to_string();
        return Task::none();
    };
    let local_path = state.import_local_path.trim().to_string();
    let vault_path = state.import_vault_path.trim().to_string();
    if local_path.is_empty() || vault_path.is_empty() {
        state.import_status = "Fill in both local file path and vault destination path".to_string();
        return Task::none();
    }
    let filename = std::path::Path::new(&vault_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| vault_path.clone());
    state.import_status = format!("Importing {}…", local_path);
    Task::perform(
        async move {
            let bytes = std::fs::read(&local_path)
                .map_err(|e| format!("Failed to read local file: {e}"))?;
            let total_size = bytes.len() as u64;
            let session = client
                .create_upload_session(
                    &vault_id,
                    &CreateUploadSessionRequest {
                        filename: filename.clone(),
                        path: vault_path.clone(),
                        total_size: Some(total_size),
                    },
                )
                .await
                .map_err(|e| format!("Failed to create upload session: {e}"))?;
            client
                .upload_chunk(&vault_id, &session.session_id, bytes)
                .await
                .map_err(|e| format!("Failed to upload chunk: {e}"))?;
            client
                .finish_upload_session(
                    &vault_id,
                    &session.session_id,
                    &FinishUploadSessionRequest {
                        filename,
                        path: vault_path.clone(),
                    },
                )
                .await
                .map_err(|e| format!("Failed to finish upload: {e}"))?;
            Ok(format!("Imported {} to {}", local_path, vault_path))
        },
        Message::FileImported,
    )
}

fn run_export_file(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        state.export_status = "Login and select a vault first".to_string();
        return Task::none();
    };
    let vault_path = state.export_vault_path.trim().to_string();
    let local_path = state.export_local_path.trim().to_string();
    if vault_path.is_empty() || local_path.is_empty() {
        state.export_status = "Fill in both vault path and local destination path".to_string();
        return Task::none();
    }
    state.export_status = format!("Exporting {}…", vault_path);
    Task::perform(
        async move {
            let bytes = client
                .download_file_bytes(&vault_id, &vault_path)
                .await
                .map_err(|e| format!("Failed to download file: {e}"))?;
            std::fs::write(&local_path, &bytes)
                .map_err(|e| format!("Failed to write local file: {e}"))?;
            Ok(format!("Exported {} bytes to {}", bytes.len(), local_path))
        },
        Message::FileExported,
    )
}

fn run_load_preferences(state: &mut DesktopApp, open_panel: bool) -> Task<Message> {
    let Some(client) = state.client.clone() else {
        state.status = "Please login first".to_string();
        return Task::none();
    };

    if open_panel {
        state.preferences_visible = true;
    }
    state.status = "Loading preferences…".to_string();

    Task::perform(
        async move {
            client
                .get_preferences()
                .await
                .map_err(|e| format!("Failed to load preferences: {e}"))
        },
        Message::PreferencesLoaded,
    )
}

fn run_save_preferences(state: &mut DesktopApp) -> Task<Message> {
    let Some(client) = state.client.clone() else {
        state.status = "Please login first".to_string();
        return Task::none();
    };

    let font_size = match state.preferences_font_size_input.trim().parse::<u16>() {
        Ok(value) if value > 0 => value,
        _ => {
            state.status = "Font size must be a positive number".to_string();
            return Task::none();
        }
    };

    let prefs = UserPreferences {
        theme: state.preferences_theme.trim().to_string(),
        editor_mode: state.preferences_editor_mode.clone(),
        font_size,
        window_layout: (!state.preferences_window_layout_input.trim().is_empty())
            .then(|| state.preferences_window_layout_input.trim().to_string()),
        icon_map: None,
    };
    state.status = "Saving preferences…".to_string();

    Task::perform(
        async move {
            client
                .update_preferences(&prefs)
                .await
                .map_err(|e| format!("Failed to save preferences: {e}"))
        },
        Message::PreferencesSaved,
    )
}

fn run_reset_preferences(state: &mut DesktopApp) -> Task<Message> {
    let Some(client) = state.client.clone() else {
        state.status = "Please login first".to_string();
        return Task::none();
    };

    state.status = "Resetting preferences…".to_string();

    Task::perform(
        async move {
            client
                .reset_preferences()
                .await
                .map_err(|e| format!("Failed to reset preferences: {e}"))
        },
        Message::PreferencesResetLoaded,
    )
}

fn parse_rfc3339_utc(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

fn is_conflict_error(err: &str) -> bool {
    err.contains("status=409") || err.to_ascii_lowercase().contains("conflict")
}

fn run_search(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        state.status = "Login and select a vault first".to_string();
        return Task::none();
    };

    let query = state.search_query.trim().to_string();
    if query.is_empty() {
        state.search_results.clear();
        state.search_total_count = 0;
        state.status = "Enter a search query".to_string();
        return Task::none();
    }

    let page = state.search_page.max(1);
    let page_size = state.search_page_size.max(1);
    state.status = format!("Searching for \"{query}\"…");

    Task::perform(
        async move {
            client
                .search(&vault_id, &query, page, page_size)
                .await
                .map_err(|e| format!("Search failed: {e}"))
        },
        Message::SearchLoaded,
    )
}

fn find_quick_switch_match(state: &DesktopApp) -> Option<String> {
    let query = state.quick_switcher_query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return None;
    }

    state
        .tree_entries
        .iter()
        .filter(|entry| !entry.is_directory)
        .find(|entry| entry.path.to_ascii_lowercase().contains(&query))
        .map(|entry| entry.path.clone())
}

fn run_outline(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        return Task::none();
    };

    let note_path = state.note_path.trim().to_string();
    if note_path.is_empty() {
        state.outline_sections.clear();
        state.outline_summary.clear();
        return Task::none();
    }

    let content = state.note_content.clone();
    let max_sections = Some(state.outline_max_sections.max(1));

    Task::perform(
        async move {
            client
                .generate_outline(&vault_id, &note_path, Some(&content), max_sections)
                .await
                .map_err(|e| format!("Outline generation failed: {e}"))
        },
        Message::OutlineLoaded,
    )
}

fn run_suggestions(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        return Task::none();
    };

    let note_path = state.note_path.trim().to_string();
    if note_path.is_empty() {
        state.suggestion_items.clear();
        state.suggestion_source_path.clear();
        return Task::none();
    }

    let content = state.note_content.clone();
    let max_suggestions = Some(state.suggestion_max_count.max(1));
    state.status = "Generating ML suggestions…".to_string();

    Task::perform(
        async move {
            client
                .generate_suggestions(&vault_id, &note_path, Some(&content), max_suggestions)
                .await
                .map_err(|e| format!("Suggestion generation failed: {e}"))
        },
        Message::SuggestionsLoaded,
    )
}

fn run_apply_suggestion(
    state: &mut DesktopApp,
    suggestion: obsidian_types::OrganizationSuggestion,
    dry_run: bool,
) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        state.status = "Login and select a vault first".to_string();
        return Task::none();
    };

    let file_path = state.note_path.trim().to_string();
    if file_path.is_empty() {
        state.status = "Load a note before applying suggestions".to_string();
        return Task::none();
    }

    let label = if dry_run { "Dry-running" } else { "Applying" };
    state.status = format!("{label} ML suggestion…");

    Task::perform(
        async move {
            client
                .apply_suggestion(
                    &vault_id,
                    &obsidian_types::ApplyOrganizationSuggestionRequest {
                        file_path,
                        suggestion,
                        dry_run,
                    },
                )
                .await
                .map_err(|e| format!("Suggestion apply failed: {e}"))
        },
        Message::SuggestionApplied,
    )
}

fn run_undo_last_ml(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id), Some(receipt_id)) = (
        state.client.clone(),
        state.selected_vault_id.clone(),
        state.last_ml_receipt_id.clone(),
    ) else {
        state.status = "No undo receipt available yet".to_string();
        return Task::none();
    };

    state.status = "Undoing last ML action…".to_string();

    Task::perform(
        async move {
            client
                .undo_ml_action(&vault_id, &receipt_id)
                .await
                .map_err(|e| format!("Undo failed: {e}"))
        },
        Message::UndoLastMlLoaded,
    )
}

fn run_backlinks(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        return Task::none();
    };

    let note_path = state.note_path.trim().to_string();
    if note_path.is_empty() {
        state.backlink_paths.clear();
        return Task::none();
    }

    Task::perform(
        async move {
            client
                .get_backlinks(&vault_id, &note_path)
                .await
                .map(|entries| entries.into_iter().map(|entry| entry.path).collect())
                .map_err(|e| format!("Backlinks lookup failed: {e}"))
        },
        Message::BacklinksLoaded,
    )
}

fn refresh_outgoing_links(state: &mut DesktopApp) {
    state.outgoing_links = parse_outgoing_links(&state.note_content, &state.note_path);
}

fn run_tags(state: &mut DesktopApp) -> Task<Message> {
    let (Some(client), Some(vault_id)) = (state.client.clone(), state.selected_vault_id.clone())
    else {
        return Task::none();
    };

    Task::perform(
        async move {
            client
                .get_tags(&vault_id)
                .await
                .map_err(|e| format!("Tag lookup failed: {e}"))
        },
        Message::TagsLoaded,
    )
}

fn parse_outgoing_links(content: &str, current_path: &str) -> Vec<String> {
    use std::collections::BTreeSet;

    let mut links = BTreeSet::new();

    let mut start = 0;
    while let Some(open_rel) = content[start..].find("[[") {
        let open = start + open_rel + 2;
        if let Some(close_rel) = content[open..].find("]]") {
            let close = open + close_rel;
            let raw = &content[open..close];
            let target = raw
                .split('|')
                .next()
                .unwrap_or("")
                .trim()
                .trim_start_matches('/');
            if !target.is_empty() && !target.starts_with('#') {
                let normalized = if target.ends_with(".md") {
                    target.to_string()
                } else {
                    format!("{target}.md")
                };
                links.insert(normalized);
            }
            start = close + 2;
        } else {
            break;
        }
    }

    let bytes = content.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b']' && bytes[i + 1] == b'(' {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b')' {
                j += 1;
            }
            if j < bytes.len() {
                let raw = &content[(i + 2)..j];
                let target = raw.trim().trim_start_matches('/');
                if !target.is_empty()
                    && !target.starts_with("http://")
                    && !target.starts_with("https://")
                    && !target.starts_with('#')
                {
                    links.insert(target.to_string());
                }
                i = j;
            }
        }
        i += 1;
    }

    let current = current_path.to_ascii_lowercase();
    links
        .into_iter()
        .filter(|link| link.to_ascii_lowercase() != current)
        .collect()
}
