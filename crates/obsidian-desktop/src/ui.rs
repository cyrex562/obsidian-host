use iced::widget::{button, column, container, image, markdown, row, scrollable, text, text_input};
use iced::{ContentFit, Element, Length, Theme};
use std::collections::BTreeSet;

use crate::state::{
    file_kind_label, is_media_file_kind, CollapsedSections, DesktopApp, DesktopMode, EditorMode,
    FileKind, TemplateInsertMode, ToolbarAction,
};
use crate::Message;

pub(crate) fn view(state: &DesktopApp) -> Element<'_, Message> {
    let auth_controls = {
        let mode_row = row![
            text("Mode:"),
            button(if state.deployment_mode == DesktopMode::Cloud {
                "● Cloud"
            } else {
                "Cloud"
            })
            .on_press(Message::DeploymentModeSelected(DesktopMode::Cloud)),
            button(if state.deployment_mode == DesktopMode::Standalone {
                "● Standalone"
            } else {
                "Standalone"
            })
            .on_press(Message::DeploymentModeSelected(DesktopMode::Standalone)),
            button(if state.deployment_mode == DesktopMode::Hybrid {
                "● Hybrid"
            } else {
                "Hybrid"
            })
            .on_press(Message::DeploymentModeSelected(DesktopMode::Hybrid)),
            text_input("Server URL (http://127.0.0.1:8080)", &state.base_url)
                .on_input(Message::BaseUrlChanged)
                .width(Length::FillPortion(3)),
            text_input("Username", &state.username)
                .on_input(Message::UsernameChanged)
                .width(Length::FillPortion(1)),
            text_input("Password", &state.password)
                .on_input(Message::PasswordChanged)
                .width(Length::FillPortion(1)),
            button("Login").on_press(Message::LoginPressed),
            button("Preferences").on_press(Message::PreferencesPressed),
            button("Plugins").on_press(Message::PluginManagerPressed),
            button("Import/Export").on_press(Message::ImportExportPressed),
            button(text(format!("Theme: {}", state.preferences_theme))).on_press(Message::CycleTheme),
            button(if state.feature_flags.diagnostics_panel {
                "● Diag"
            } else {
                "Diag"
            })
            .on_press(Message::DiagnosticsPanelToggled),
            button(if state.event_sync_requested {
                "Stop Sync"
            } else {
                "WS Sync"
            })
            .on_press(if state.event_sync_requested {
                Message::DisconnectEventsPressed
            } else {
                Message::ConnectEventsPressed
            }),
        ]
        .spacing(8);

        let col = column![mode_row].spacing(4);
        if state.deployment_mode == DesktopMode::Hybrid {
            col.push(
                row![
                    text("Local mirror URL:").size(13),
                    text_input("http://localhost:8080", &state.local_base_url)
                        .on_input(Message::LocalBaseUrlChanged)
                        .width(Length::Fill),
                ]
                .spacing(8),
            )
        } else {
            col
        }
    };

    let preferences_panel = if state.preferences_visible {
        container(view_preferences_panel(state)).padding(8)
    } else {
        container(column![])
    };

    let plugin_panel = if state.plugin_panel_visible {
        container(view_plugin_manager_panel(state)).padding(8)
    } else {
        container(column![])
    };

    let import_export_panel = if state.import_export_visible {
        container(view_import_export_panel(state)).padding(8)
    } else {
        container(column![])
    };

    let diagnostics_panel = if state.feature_flags.diagnostics_panel {
        container(view_diagnostics_panel(state)).padding(8)
    } else {
        container(column![])
    };

    let body = row![
        container(view_sidebar(state))
            .width(Length::FillPortion(1))
            .padding(8),
        container(view_editor_workspace(state))
            .width(Length::FillPortion(3))
            .padding(8),
    ]
    .spacing(10)
    .height(Length::Fill);

    container(
        column![
            auth_controls,
            preferences_panel,
            plugin_panel,
            import_export_panel,
            diagnostics_panel,
            body,
            view_status_footer(state)
        ]
        .spacing(10)
        .padding(10),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn view_sidebar(state: &DesktopApp) -> Element<'_, Message> {
    let vault_buttons = state
        .vaults
        .iter()
        .fold(column![text("Vaults")].spacing(6), |col, vault| {
            let label = if state.selected_vault_id.as_deref() == Some(vault.id.as_str()) {
                format!("• {}", vault.name)
            } else {
                vault.name.clone()
            };
            col.push(button(text(label)).on_press(Message::VaultSelected(vault.id.clone())))
        })
        .push(button("Refresh Vaults").on_press(Message::LoadVaultsPressed))
        .push(
            row![
                text_input("New vault name", &state.new_vault_name)
                    .on_input(Message::NewVaultNameChanged)
                    .width(Length::Fill),
                button("+ Vault").on_press(Message::CreateVaultPressed),
            ]
            .spacing(6),
        )
        .push(button("Refresh Tree").on_press(Message::LoadTreePressed));

    let tree_header = row![
        text("Vault Tree"),
        button(if state.tree_sort_ascending { "A→Z" } else { "Z→A" })
            .on_press(Message::ToggleTreeSort),
        button(if state.collapsed_sections.file_tree { "▶" } else { "▼" })
            .on_press(Message::ToggleSidebarSection("file_tree".to_string())),
    ]
    .spacing(6);

    let tree_panel = if state.collapsed_sections.file_tree {
        scrollable(column![tree_header].spacing(4)).height(40)
    } else {
        scrollable(state.tree_entries.iter().fold(
            column![tree_header].spacing(4),
            |col, entry| {
                let prefix = if state.selected_tree_path.as_deref() == Some(entry.path.as_str()) {
                    "• "
                } else {
                    ""
                };

                let label = if entry.is_directory {
                    format!("{prefix}📁 {}", entry.display)
                } else {
                    format!("{prefix}📄 {}", entry.display)
                };

                col.push(
                    button(text(label))
                        .width(Length::Fill)
                        .on_press(Message::TreeEntrySelected(entry.path.clone())),
                )
            },
        ))
        .height(Length::Fill)
    };

    let quick_create = column![
        text("Quick Actions"),
        row![
            button("Daily Note").on_press(Message::DailyNotePressed),
            button("Random Note").on_press(Message::RandomNotePressed),
        ]
        .spacing(6),
        row![
            text_input("notes/new-note.md", &state.new_file_path)
                .on_input(Message::NewFilePathChanged)
                .width(Length::Fill),
            button("New Note").on_press(Message::CreateFilePressed),
        ]
        .spacing(6),
        row![
            text_input("projects/new-folder", &state.new_folder_path)
                .on_input(Message::NewFolderPathChanged)
                .width(Length::Fill),
            button("New Folder").on_press(Message::CreateFolderPressed),
        ]
        .spacing(6),
        row![
            text_input("from/path.md", &state.rename_from_path)
                .on_input(Message::RenameFromPathChanged)
                .width(Length::Fill),
            text_input("to/path.md", &state.rename_to_path)
                .on_input(Message::RenameToPathChanged)
                .width(Length::Fill),
            button("Rename / Move").on_press(Message::RenamePathPressed),
        ]
        .spacing(6),
        row![
            text_input("path/to/delete.md", &state.delete_target_path)
                .on_input(Message::DeleteTargetPathChanged)
                .width(Length::FillPortion(2)),
            button(if state.delete_confirmation_armed {
                "Confirm Delete"
            } else {
                "Arm Delete"
            })
            .on_press(if state.delete_confirmation_armed {
                Message::DeletePathPressed
            } else {
                Message::ArmDeletePressed
            }),
            button("Cancel").on_press(Message::DeleteCanceled),
        ]
        .spacing(6),
    ]
    .spacing(6);

    let recent_files = if state.recent_files.is_empty() {
        column![text("Recent Files"), text("No recent files yet")].spacing(4)
    } else {
        state
            .recent_files
            .iter()
            .fold(column![text("Recent Files")].spacing(4), |col, path| {
                col.push(
                    button(text(path.as_str()))
                        .width(Length::Fill)
                        .on_press(Message::QuickReopenPressed(path.clone())),
                )
            })
    };

    let bookmarks_panel = if state.bookmarks.is_empty() {
        column![text("Bookmarks"), text("No bookmarks yet")].spacing(4)
    } else {
        state
            .bookmarks
            .iter()
            .fold(column![text("Bookmarks")].spacing(4), |col, path| {
                col.push(
                    button(text(path.as_str()))
                        .width(Length::Fill)
                        .on_press(Message::QuickReopenPressed(path.clone())),
                )
            })
    };

    let tags_panel = if state.tag_entries.is_empty() {
        column![
            row![
                text("Tags"),
                button("Refresh Tags").on_press(Message::LoadTagsPressed),
            ]
            .spacing(6),
            text("No tags loaded").size(12),
        ]
        .spacing(4)
    } else {
        let mut panel = column![row![
            text("Tags"),
            button("Refresh Tags").on_press(Message::LoadTagsPressed),
        ]
        .spacing(6)]
        .spacing(4);

        for tag in state.tag_entries.iter().take(12) {
            panel = panel.push(
                button(text(format!("#{} ({})", tag.tag, tag.count)).size(12))
                    .width(Length::Fill)
                    .on_press(Message::TagSearchPressed(tag.tag.clone())),
            );
        }

        panel
    };

    let search_controls = column![
        text("Search"),
        row![
            text_input("Search notes", &state.search_query)
                .on_input(Message::SearchQueryChanged)
                .width(Length::Fill),
            button("Search").on_press(Message::SearchPressed),
        ]
        .spacing(6),
        row![
            button("Prev").on_press(Message::SearchPrevPagePressed),
            text(format!(
                "Page {} · {} total",
                state.search_page, state.search_total_count
            )),
            button("Next").on_press(Message::SearchNextPagePressed),
        ]
        .spacing(6),
    ]
    .spacing(6);

    let quick_switch_query = state.quick_switcher_query.trim().to_ascii_lowercase();
    let quick_switch_matches: Vec<_> = if quick_switch_query.is_empty() {
        Vec::new()
    } else {
        state
            .tree_entries
            .iter()
            .filter(|entry| !entry.is_directory)
            .filter(|entry| {
                entry
                    .path
                    .to_ascii_lowercase()
                    .contains(&quick_switch_query)
            })
            .take(8)
            .collect()
    };

    let quick_switcher_panel = if quick_switch_matches.is_empty() {
        column![
            text("Quick Switcher"),
            row![
                text_input("Jump to note path", &state.quick_switcher_query)
                    .on_input(Message::QuickSwitcherQueryChanged)
                    .on_submit(Message::QuickSwitcherOpenFirstPressed)
                    .width(Length::Fill),
                button("Open First").on_press(Message::QuickSwitcherOpenFirstPressed),
            ]
            .spacing(6),
            text("Type to see matching notes").size(12),
        ]
        .spacing(6)
    } else {
        let matches_column = quick_switch_matches.into_iter().fold(
            column![text("Quick Switcher")].spacing(4),
            |col, entry| {
                col.push(
                    button(text(entry.path.as_str()))
                        .width(Length::Fill)
                        .on_press(Message::QuickReopenPressed(entry.path.clone())),
                )
            },
        );

        matches_column.push(
            row![
                text_input("Jump to note path", &state.quick_switcher_query)
                    .on_input(Message::QuickSwitcherQueryChanged)
                    .on_submit(Message::QuickSwitcherOpenFirstPressed)
                    .width(Length::Fill),
                button("Open First").on_press(Message::QuickSwitcherOpenFirstPressed),
            ]
            .spacing(6),
        )
    };

    let search_results_panel = if state.search_results.is_empty() {
        column![text("Search Results"), text("No results yet")].spacing(4)
    } else {
        state.search_results.iter().fold(
            column![text("Search Results")].spacing(4),
            |col, result| {
                let snippet = result
                    .matches
                    .first()
                    .map(|m| format!("L{}: {}", m.line_number, m.line_text.trim()))
                    .unwrap_or_else(|| "No snippet".to_string());

                col.push(
                    container(
                        column![
                            button(text(result.path.as_str()))
                                .width(Length::Fill)
                                .on_press(Message::QuickReopenPressed(result.path.clone())),
                            text(snippet).size(12),
                        ]
                        .spacing(2),
                    )
                    .padding(4),
                )
            },
        )
    };

    column![
        vault_buttons,
        quick_switcher_panel,
        search_controls,
        search_results_panel,
        quick_create,
        bookmarks_panel,
        tags_panel,
        recent_files,
        tree_panel
    ]
    .spacing(10)
    .into()
}

fn view_editor_workspace(state: &DesktopApp) -> Element<'_, Message> {
    let mode_controls = row![
        text("Mode:"),
        button(if state.editor_mode == EditorMode::Raw {
            "● Raw"
        } else {
            "Raw"
        })
        .on_press(Message::EditorModeSelected(EditorMode::Raw)),
        button(if state.editor_mode == EditorMode::Formatted {
            "● Formatted"
        } else {
            "Formatted"
        })
        .on_press(Message::EditorModeSelected(EditorMode::Formatted)),
        button(if state.editor_mode == EditorMode::Preview {
            "● Preview"
        } else {
            "Preview"
        })
        .on_press(Message::EditorModeSelected(EditorMode::Preview)),
    ]
    .spacing(6);

    let toolbar = row![
        text("Toolbar:"),
        button("H1").on_press(Message::ToolbarActionPressed(ToolbarAction::Heading)),
        button("Bold").on_press(Message::ToolbarActionPressed(ToolbarAction::Bold)),
        button("Italic").on_press(Message::ToolbarActionPressed(ToolbarAction::Italic)),
        button("List").on_press(Message::ToolbarActionPressed(ToolbarAction::BulletList)),
        button("Quote").on_press(Message::ToolbarActionPressed(ToolbarAction::Quote)),
        button("Code").on_press(Message::ToolbarActionPressed(ToolbarAction::CodeFence)),
        button("Refresh Preview").on_press(Message::RenderPreviewRequested),
    ]
    .spacing(6);

    let note_controls = row![
        text_input("path/to/note.md", &state.note_path)
            .on_input(Message::NotePathChanged)
            .width(Length::Fill),
        button("Open").on_press(Message::LoadNotePressed),
        button("Save").on_press(Message::SaveNotePressed),
        button("Force Save").on_press(Message::SaveNoteForcePressed),
        button("Bookmark").on_press(Message::ToggleBookmarkPressed),
        button(if state.split_pane_enabled {
            "Close Split"
        } else {
            "Split Pane"
        })
        .on_press(Message::ToggleSplitPane),
    ]
    .spacing(8);

    if is_media_file_kind(state.current_file_kind) {
        let media_body: Element<'_, Message> = if state.feature_flags.media_preview {
            view_media_workspace(state)
        } else {
            container(
                text("Media preview is disabled — enable in the Diagnostics panel").size(13),
            )
            .padding(12)
            .into()
        };
        return column![
            view_tab_strip(state),
            view_details_header(state),
            note_controls,
            media_body,
        ]
        .spacing(8)
        .into();
    }

    let template_controls = row![
        text("Template:"),
        text_input("Templates/Daily Note.md", &state.template_path)
            .on_input(Message::TemplatePathChanged)
            .width(Length::FillPortion(2)),
        button(
            if state.template_insert_mode == TemplateInsertMode::Append {
                "● Append"
            } else {
                "Append"
            }
        )
        .on_press(Message::TemplateModeSelected(TemplateInsertMode::Append)),
        button(
            if state.template_insert_mode == TemplateInsertMode::Replace {
                "● Replace"
            } else {
                "Replace"
            }
        )
        .on_press(Message::TemplateModeSelected(TemplateInsertMode::Replace)),
        button("Insert Template").on_press(Message::InsertTemplatePressed),
    ]
    .spacing(6);

    let conflict_panel: Element<'_, Message> = if state.conflict_active {
        container(
            column![
                text("⚠ Save Conflict Detected").size(16),
                text(state.conflict_message.as_str()).size(13),
                text(
                    "Reload pulls latest disk version into editor. Force Save overwrites disk with your local content.",
                )
                .size(12),
                row![
                    button("Reload From Disk").on_press(Message::ConflictReloadPressed),
                    button("Force Save Local").on_press(Message::SaveNoteForcePressed),
                    button("Dismiss").on_press(Message::ConflictDismissed),
                ]
                .spacing(6),
            ]
            .spacing(6),
        )
        .padding(10)
        .into()
    } else {
        container(text("No active save conflicts").size(12))
            .padding(6)
            .into()
    };

    let outline_panel: Element<'_, Message> = if !state.feature_flags.ml_features {
        container(text("ML features disabled — enable in Diagnostics panel").size(11))
            .padding(4)
            .into()
    } else if state.outline_sections.is_empty() {
        container(
            column![
                row![
                    text("Outline"),
                    button("Refresh Outline").on_press(Message::OutlineRefreshPressed),
                ]
                .spacing(6),
                text(if state.note_path.trim().is_empty() {
                    "Load a note to generate an outline"
                } else {
                    "No headings found yet"
                })
                .size(12),
            ]
            .spacing(6),
        )
        .padding(8)
        .into()
    } else {
        let sections = state.outline_sections.iter().fold(
            column![
                row![
                    text("Outline"),
                    button("Refresh Outline").on_press(Message::OutlineRefreshPressed),
                ]
                .spacing(6),
                text(state.outline_summary.as_str()).size(12),
            ]
            .spacing(4),
            |col, section| {
                let indent = "  ".repeat(section.level.saturating_sub(1) as usize);
                let label = format!("{indent}L{} {}", section.line_number, section.title);
                col.push(button(text(label)).width(Length::Fill).on_press(
                    Message::OutlineSectionSelected(section.line_number, section.title.clone()),
                ))
            },
        );

        container(scrollable(sections).height(160))
            .padding(8)
            .into()
    };

    let suggestions_panel: Element<'_, Message> = if !state.feature_flags.ml_features {
        container(column![]).into()
    } else if state.suggestion_items.is_empty() {
        let undo_button = if state.last_ml_receipt_id.is_some() {
            button("Undo Last").on_press(Message::UndoLastMlPressed)
        } else {
            button("Undo Last")
        };

        container(
            column![
                row![
                    text("ML Suggestions"),
                    button("Generate Suggestions").on_press(Message::SuggestionsRefreshPressed),
                    undo_button,
                ]
                .spacing(6),
                text(if state.note_path.trim().is_empty() {
                    "Load a note to generate suggestions"
                } else {
                    "No suggestions yet"
                })
                .size(12),
                text(state.last_ml_action_summary.as_str()).size(12),
            ]
            .spacing(6),
        )
        .padding(8)
        .into()
    } else {
        let undo_button = if state.last_ml_receipt_id.is_some() {
            button("Undo Last").on_press(Message::UndoLastMlPressed)
        } else {
            button("Undo Last")
        };

        let suggestions = state.suggestion_items.iter().enumerate().fold(
            column![
                row![
                    text("ML Suggestions"),
                    button("Generate Suggestions").on_press(Message::SuggestionsRefreshPressed),
                    undo_button,
                ]
                .spacing(6),
                text(format!("Source: {}", state.suggestion_source_path)).size(12),
                text(state.last_ml_action_summary.as_str()).size(12),
            ]
            .spacing(4),
            |col, (index, suggestion)| {
                let kind = format!("{:?}", suggestion.kind);
                let confidence = format!("{:.0}%", suggestion.confidence * 100.0);
                let target = suggestion
                    .tag
                    .as_ref()
                    .map(|v| format!("tag: #{v}"))
                    .or_else(|| {
                        suggestion
                            .category
                            .as_ref()
                            .map(|v| format!("category: {v}"))
                    })
                    .or_else(|| {
                        suggestion
                            .target_folder
                            .as_ref()
                            .map(|v| format!("move to: {v}"))
                    })
                    .unwrap_or_else(|| "no target data".to_string());

                col.push(
                    container(
                        column![
                            text(format!("{kind} · {confidence}")),
                            text(target).size(12),
                            text(suggestion.rationale.as_str()).size(12),
                            row![
                                button("Dry Run")
                                    .on_press(Message::SuggestionActionPressed(index, true)),
                                button("Apply")
                                    .on_press(Message::SuggestionActionPressed(index, false)),
                            ]
                            .spacing(6),
                        ]
                        .spacing(2),
                    )
                    .padding(6),
                )
            },
        );

        container(scrollable(suggestions).height(170))
            .padding(8)
            .into()
    };

    let neighboring_files: Vec<String> = {
        let mut set = BTreeSet::new();
        for path in &state.outgoing_links {
            set.insert(path.clone());
        }
        for path in &state.backlink_paths {
            set.insert(path.clone());
        }
        set.into_iter().collect()
    };

    let outgoing_panel = if state.outgoing_links.is_empty() {
        column![text("Outgoing Links"), text("No outgoing links").size(12)].spacing(4)
    } else {
        state
            .outgoing_links
            .iter()
            .fold(column![text("Outgoing Links")].spacing(4), |col, path| {
                col.push(
                    button(text(path.as_str()))
                        .width(Length::Fill)
                        .on_press(Message::QuickReopenPressed(path.clone())),
                )
            })
    };

    let backlinks_panel = if state.backlink_paths.is_empty() {
        column![
            row![
                text("Backlinks"),
                button("Refresh Backlinks").on_press(Message::BacklinksRefreshPressed),
            ]
            .spacing(6),
            text("No backlinks").size(12),
        ]
        .spacing(4)
    } else {
        state.backlink_paths.iter().fold(
            column![row![
                text("Backlinks"),
                button("Refresh Backlinks").on_press(Message::BacklinksRefreshPressed),
            ]
            .spacing(6),]
            .spacing(4),
            |col, path| {
                col.push(
                    button(text(path.as_str()))
                        .width(Length::Fill)
                        .on_press(Message::QuickReopenPressed(path.clone())),
                )
            },
        )
    };

    let neighboring_panel = if neighboring_files.is_empty() {
        column![
            text("Neighboring Files"),
            text("No adjacent notes yet").size(12)
        ]
        .spacing(4)
    } else {
        neighboring_files.into_iter().fold(
            column![text("Neighboring Files")].spacing(4),
            |col, path| {
                col.push(
                    button(text(path.clone()))
                        .width(Length::Fill)
                        .on_press(Message::QuickReopenPressed(path.clone())),
                )
            },
        )
    };

    let links_panel: Element<'_, Message> =
        container(column![outgoing_panel, backlinks_panel, neighboring_panel].spacing(8))
            .padding(8)
            .into();

    let editor = text_input("Note content", &state.note_content)
        .on_input(Message::EditorChanged)
        .width(Length::Fill)
        .padding(10)
        .size(
            state
                .preferences_font_size_input
                .parse::<u16>()
                .unwrap_or(14),
        );

    let frontmatter_panel = container(
        column![
            text("Frontmatter (JSON)"),
            text_input("{\"tags\": [\"project\"]}", &state.note_frontmatter_raw)
                .on_input(Message::FrontmatterChanged)
                .width(Length::Fill),
            text(state.note_frontmatter_summary.as_str()).size(13),
        ]
        .spacing(4),
    )
    .padding(8);

    let preview_status = if let Some(err) = &state.preview_render_error {
        format!("Preview error: {err}")
    } else if state.preview_markdown.is_empty() {
        "Preview is empty. Edit the note or click Refresh Preview.".to_string()
    } else {
        "Rendered markdown preview:".to_string()
    };

    let preview_markdown = markdown::view(
        state.preview_markdown.iter(),
        markdown::Settings::default(),
        markdown::Style::from_palette(Theme::TokyoNight.palette()),
    )
    .map(|url| Message::PreviewLinkClicked(url.to_string()));

    let preview = scrollable(
        container(
            column![
                text(preview_status).size(13),
                text(format!(
                    "Preview font size: {}",
                    state.preferences_font_size_input
                ))
                .size(12),
                preview_markdown
            ]
            .spacing(6),
        )
        .padding(10),
    )
    .height(Length::Fill);

    let primary_pane: Element<'_, Message> = match state.editor_mode {
        EditorMode::Raw => container(editor).into(),
        EditorMode::Formatted => row![
            container(editor).width(Length::FillPortion(1)),
            container(column![text("Preview"), preview].spacing(6)).width(Length::FillPortion(1)),
        ]
        .spacing(8)
        .into(),
        EditorMode::Preview => container(column![text("Preview"), preview].spacing(6)).into(),
    };

    // Build the workspace — either single pane or split.
    let workspace_body: Element<'_, Message> = if state.split_pane_enabled {
        // Build a tab selector for the right pane and a read-only preview of the selected tab.
        let split_tab_choices = state.open_tabs.iter().filter(|tab| {
            state.active_tab_path.as_deref() != Some(tab.path.as_str())
        });
        let split_tabs = split_tab_choices.fold(
            column![text("Right Pane")].spacing(4),
            |col, tab| {
                let is_active = state.split_pane_active_tab.as_deref() == Some(tab.path.as_str());
                let label = if is_active {
                    format!("• {}", tab.title)
                } else {
                    tab.title.clone()
                };
                col.push(
                    button(text(label))
                        .width(Length::Fill)
                        .on_press(Message::SplitPaneTabSelected(tab.path.clone())),
                )
            },
        );

        let split_content: Element<'_, Message> =
            if let Some(split_tab) = state.split_pane_active_tab.as_deref().and_then(|path| {
                state.open_tabs.iter().find(|t| t.path == path)
            }) {
                let content_preview = text_input("", &split_tab.content)
                    .width(Length::Fill)
                    .padding(10)
                    .size(
                        state
                            .preferences_font_size_input
                            .parse::<u16>()
                            .unwrap_or(14),
                    );

                container(
                    column![
                        text(format!("Split: {}", split_tab.title)).size(14),
                        content_preview,
                    ]
                    .spacing(6),
                )
                .padding(8)
                .height(Length::Fill)
                .into()
            } else {
                container(text("Select a tab for the right pane"))
                    .padding(12)
                    .into()
            };

        row![
            container(primary_pane).width(Length::FillPortion(1)),
            container(column![split_tabs, split_content].spacing(6))
                .width(Length::FillPortion(1))
                .padding(4),
        ]
        .spacing(8)
        .into()
    } else {
        primary_pane
    };

    column![
        view_tab_strip(state),
        view_details_header(state),
        mode_controls,
        toolbar,
        frontmatter_panel,
        outline_panel,
        suggestions_panel,
        links_panel,
        note_controls,
        template_controls,
        conflict_panel,
        workspace_body
    ]
    .spacing(8)
    .into()
}

fn view_preferences_panel(state: &DesktopApp) -> Element<'_, Message> {
    let theme_row = row![
        text("Theme:"),
        button(if state.preferences_theme == "dark" {
            "● Dark"
        } else {
            "Dark"
        })
        .on_press(Message::PreferencesThemeChanged("dark".to_string())),
        button(if state.preferences_theme == "light" {
            "● Light"
        } else {
            "Light"
        })
        .on_press(Message::PreferencesThemeChanged("light".to_string())),
        button(if state.preferences_theme == "system" {
            "● System"
        } else {
            "System"
        })
        .on_press(Message::PreferencesThemeChanged("system".to_string())),
    ]
    .spacing(6);

    let editor_mode_row = row![
        text("Editor mode:"),
        button(
            if matches!(
                state.preferences_editor_mode,
                obsidian_types::EditorMode::Raw
            ) {
                "● Raw"
            } else {
                "Raw"
            }
        )
        .on_press(Message::PreferencesEditorModeSelected(
            obsidian_types::EditorMode::Raw
        )),
        button(
            if matches!(
                state.preferences_editor_mode,
                obsidian_types::EditorMode::SideBySide
            ) {
                "● SideBySide"
            } else {
                "SideBySide"
            }
        )
        .on_press(Message::PreferencesEditorModeSelected(
            obsidian_types::EditorMode::SideBySide
        )),
        button(
            if matches!(
                state.preferences_editor_mode,
                obsidian_types::EditorMode::FormattedRaw
            ) {
                "● FormattedRaw"
            } else {
                "FormattedRaw"
            }
        )
        .on_press(Message::PreferencesEditorModeSelected(
            obsidian_types::EditorMode::FormattedRaw,
        )),
        button(
            if matches!(
                state.preferences_editor_mode,
                obsidian_types::EditorMode::FullyRendered
            ) {
                "● FullyRendered"
            } else {
                "FullyRendered"
            }
        )
        .on_press(Message::PreferencesEditorModeSelected(
            obsidian_types::EditorMode::FullyRendered,
        )),
    ]
    .spacing(6);

    container(
        column![
            row![
                text("Preferences").size(18),
                button("Reload").on_press(Message::PreferencesPressed),
                button("Save").on_press(Message::PreferencesSavePressed),
                button("Reset").on_press(Message::PreferencesResetPressed),
                button("Close").on_press(Message::PreferencesClosed),
            ]
            .spacing(8),
            theme_row,
            editor_mode_row,
            row![
                text("Font size:"),
                text_input("14", &state.preferences_font_size_input)
                    .on_input(Message::PreferencesFontSizeChanged)
                    .width(80),
            ]
            .spacing(6),
            row![
                text("Window layout:"),
                text_input("optional layout preset", &state.preferences_window_layout_input)
                    .on_input(Message::PreferencesWindowLayoutChanged)
                    .width(Length::Fill),
            ]
            .spacing(6),
            text("Theme is saved remotely; desktop currently keeps the TokyoNight shell while applying editor mode and font size locally.").size(12),
        ]
        .spacing(8),
    )
    .into()
}

fn view_tab_strip(state: &DesktopApp) -> Element<'_, Message> {
    if state.open_tabs.is_empty() {
        return container(text("No open tabs")).padding(8).into();
    }

    let tabs = state.open_tabs.iter().fold(row![].spacing(6), |row, tab| {
        let is_active = state.active_tab_path.as_deref() == Some(tab.path.as_str());
        let dirty_marker = if tab.is_dirty { "*" } else { "" };
        let title = if is_active {
            format!("• {}{}", tab.title, dirty_marker)
        } else {
            format!("{}{}", tab.title, dirty_marker)
        };

        row.push(
            container(
                row![
                    button(text(title)).on_press(Message::TabSelected(tab.path.clone())),
                    button(text("×")).on_press(Message::TabClosed(tab.path.clone())),
                ]
                .spacing(4),
            )
            .padding(4),
        )
    });

    container(scrollable(tabs).height(50)).padding(4).into()
}

fn view_details_header(state: &DesktopApp) -> Element<'_, Message> {
    let dirty_label = if state.note_is_dirty {
        "Modified"
    } else {
        "Saved"
    };
    let modified_label = state
        .note_modified
        .clone()
        .unwrap_or_else(|| "Not loaded yet".to_string());

    container(
        column![
            text("Selected Note").size(18),
            text(format!("Open tabs: {}", state.open_tabs.len())),
            text(format!(
                "Path: {}",
                if state.note_path.is_empty() {
                    "—"
                } else {
                    state.note_path.as_str()
                }
            )),
            text(format!("State: {dirty_label}")),
            text(format!("Modified: {modified_label}")),
            text(state.note_frontmatter_summary.as_str()),
        ]
        .spacing(4),
    )
    .padding(10)
    .into()
}

fn view_status_footer(state: &DesktopApp) -> Element<'_, Message> {
    let active_vault = state
        .vaults
        .iter()
        .find(|vault| state.selected_vault_id.as_deref() == Some(vault.id.as_str()))
        .map(|vault| vault.name.as_str())
        .unwrap_or("No vault selected");
    let active_tab = state.active_tab_path.as_deref().unwrap_or("No active tab");
    let save_state = if state.note_is_dirty {
        "Unsaved changes"
    } else {
        "All changes saved"
    };
    let sync_state = if state.event_sync_connected {
        "Connected"
    } else if state.event_sync_requested {
        "Reconnecting"
    } else {
        "Disconnected"
    };

    let word_count = if state.note_content.is_empty() {
        0
    } else {
        state.note_content.split_whitespace().count()
    };
    let char_count = state.note_content.len();

    container(
        row![
            text(format!("Vault: {active_vault}")),
            text(format!("Tabs: {}", state.open_tabs.len())),
            text(format!("Active: {active_tab}")),
            text(save_state),
            text(format!("{word_count} words · {char_count} chars")),
            text(format!("Sync: {sync_state}")),
            text(format!("Sync detail: {}", state.event_sync_last_message)),
            text(format!("Status: {}", state.status)),
        ]
        .spacing(12),
    )
    .padding(8)
    .width(Length::Fill)
    .into()
}

fn view_plugin_manager_panel(state: &DesktopApp) -> Element<'_, Message> {
    let plugin_rows: Element<'_, Message> = if state.plugins.is_empty() {
        let body_text =
            if state.plugin_status.is_empty() || state.plugin_status.starts_with("Loading") {
                state.plugin_status.as_str()
            } else {
                "No plugins found. Make sure the plugins/ directory contains plugin manifests."
            };
        column![text(body_text).size(12)].spacing(4).into()
    } else {
        let list = state
            .plugins
            .iter()
            .fold(column![].spacing(6), |col, plugin| {
                let toggle_label = if plugin.enabled { "Disable" } else { "Enable" };
                let state_badge = format!("[{}]", plugin.state_label);
                let id_desc = if plugin.description.is_empty() {
                    format!("ID: {}", plugin.id)
                } else {
                    format!("ID: {}  —  {}", plugin.id, plugin.description)
                };

                let row_items = row![
                    text(format!("{} v{}", plugin.name, plugin.version)),
                    text(state_badge).size(12),
                    button(toggle_label).on_press(Message::TogglePluginPressed(
                        plugin.id.clone(),
                        !plugin.enabled
                    )),
                ]
                .spacing(8);

                let mut details = column![row_items, text(id_desc).size(12)].spacing(2);

                if let Some(ref err) = plugin.last_error {
                    details = details.push(text(format!("Error: {err}")).size(11));
                }

                col.push(container(details).padding(6))
            });

        scrollable(list).height(220).into()
    };

    container(
        column![
            row![
                text("Plugin Manager").size(18),
                button("Reload").on_press(Message::PluginsRefreshPressed),
                button("Close").on_press(Message::PluginManagerPressed),
            ]
            .spacing(8),
            text(state.plugin_status.as_str()).size(12),
            plugin_rows,
        ]
        .spacing(8),
    )
    .padding(8)
    .width(Length::Fill)
    .into()
}

fn view_import_export_panel(state: &DesktopApp) -> Element<'_, Message> {
    let import_section = column![
        text("Import file into vault").size(14),
        row![
            text("Local file path:").width(Length::FillPortion(1)),
            text_input("e.g. C:\\notes\\file.md", &state.import_local_path)
                .on_input(Message::ImportLocalPathChanged)
                .width(Length::FillPortion(3)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        row![
            text("Vault destination path:").width(Length::FillPortion(1)),
            text_input("e.g. folder/note.md", &state.import_vault_path)
                .on_input(Message::ImportVaultPathChanged)
                .width(Length::FillPortion(3)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        row![
            button("Import").on_press(Message::ImportFilePressed),
            text(state.import_status.as_str()).size(12),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(8);

    let export_section = column![
        text("Export file from vault").size(14),
        row![
            text("Vault file path:").width(Length::FillPortion(1)),
            text_input("e.g. folder/note.md", &state.export_vault_path)
                .on_input(Message::ExportVaultPathChanged)
                .width(Length::FillPortion(3)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        row![
            text("Local save path:").width(Length::FillPortion(1)),
            text_input("e.g. C:\\downloads\\note.md", &state.export_local_path)
                .on_input(Message::ExportLocalPathChanged)
                .width(Length::FillPortion(3)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        row![
            button("Export").on_press(Message::ExportFilePressed),
            text(state.export_status.as_str()).size(12),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(8);

    container(
        column![
            row![
                text("Import / Export").size(16),
                button("Close").on_press(Message::ImportExportPressed),
            ]
            .spacing(16)
            .align_y(iced::Alignment::Center),
            row![
                container(import_section)
                    .padding(8)
                    .width(Length::FillPortion(1)),
                container(export_section)
                    .padding(8)
                    .width(Length::FillPortion(1)),
            ]
            .spacing(16),
        ]
        .spacing(12),
    )
    .padding(8)
    .width(Length::Fill)
    .into()
}

fn view_media_workspace(state: &DesktopApp) -> Element<'_, Message> {
    let headline = format!(
        "{} viewer",
        file_kind_label(state.current_file_kind)
    );

    let actions = row![
        button("Open Externally").on_press(Message::OpenMediaExternallyPressed),
        button("Reload").on_press(Message::LoadNotePressed),
        text(state.media_status.as_str()).size(12),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let viewer: Element<'_, Message> = match state.current_file_kind {
        FileKind::Image => {
            if let Some(handle) = state.media_image.clone() {
                container(
                    image(handle)
                        .content_fit(ContentFit::Contain)
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            } else {
                container(text("Image preview is still loading or unavailable.")).into()
            }
        }
        FileKind::Pdf => container(
            column![
                text("PDF Document").size(16),
                text("Use \"Open Externally\" to view in your system PDF reader.").size(13),
                text(format!("File: {}", state.note_path)).size(12),
                button("Open in System PDF Viewer").on_press(Message::OpenMediaExternallyPressed),
            ]
            .spacing(8),
        )
        .padding(16)
        .into(),
        FileKind::Audio => container(
            column![
                text("Audio File").size(16),
                text("Use \"Open Externally\" to play in your system audio player.").size(13),
                text(format!("File: {}", state.note_path)).size(12),
                button("Play in System Audio Player").on_press(Message::OpenMediaExternallyPressed),
            ]
            .spacing(8),
        )
        .padding(16)
        .into(),
        FileKind::Video => container(
            column![
                text("Video File").size(16),
                text("Use \"Open Externally\" to play in your system video player.").size(13),
                text(format!("File: {}", state.note_path)).size(12),
                button("Play in System Video Player").on_press(Message::OpenMediaExternallyPressed),
            ]
            .spacing(8),
        )
        .padding(16)
        .into(),
        FileKind::Other => container(
            column![
                text("Binary File").size(16),
                text("This file type cannot be previewed. Use \"Open Externally\" to view it.").size(13),
                text(format!("File: {}", state.note_path)).size(12),
                button("Open Externally").on_press(Message::OpenMediaExternallyPressed),
            ]
            .spacing(8),
        )
        .padding(16)
        .into(),
        FileKind::Markdown | FileKind::Text => container(text("Not a media file.")).into(),
    };

    container(
        column![
            text(headline).size(18),
            text(format!("Path: {}", state.note_path)).size(12),
            if state.media_source_url.is_empty() {
                text("No media source URL resolved yet.").size(12)
            } else {
                text(format!("Source: {}", state.media_source_url)).size(12)
            },
            actions,
            container(viewer).padding(8).height(Length::Fill),
        ]
        .spacing(8),
    )
    .padding(10)
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

fn view_diagnostics_panel(state: &DesktopApp) -> Element<'_, Message> {
    let flags = &state.feature_flags;
    let diag = &state.diagnostics;

    let flag_row = row![
        text("Feature flags:"),
        button(if flags.ml_features { "● ML on" } else { "ML off" })
            .on_press(Message::FeatureFlagMlChanged(!flags.ml_features)),
        button(if flags.media_preview {
            "● Media on"
        } else {
            "Media off"
        })
        .on_press(Message::FeatureFlagMediaChanged(!flags.media_preview)),
        button(if flags.event_sync {
            "● Sync on"
        } else {
            "Sync off"
        })
        .on_press(Message::FeatureFlagSyncChanged(!flags.event_sync)),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let counters = row![
        text(format!("Notes loaded: {}", diag.notes_loaded)),
        text(format!("Notes saved: {}", diag.notes_saved)),
        text(format!("ML requests: {}", diag.ml_requests)),
        text(format!("Sync msgs: {}", diag.sync_messages_received)),
        text(format!("Errors: {}", diag.errors_logged)),
        button("Copy Diagnostics").on_press(Message::CopyDiagnosticsPressed),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);

    // Show the most-recent 8 log lines in a fixed-height scrollable
    let recent_log = scrollable(
        diag.log_lines
            .iter()
            .rev()
            .take(8)
            .fold(column![].spacing(2), |col, line| {
                col.push(text(line.as_str()).size(11))
            }),
    )
    .height(80);

    container(
        column![
            text("Diagnostics").size(16),
            flag_row,
            counters,
            text("Recent log (newest first):").size(12),
            recent_log,
        ]
        .spacing(8),
    )
    .padding(10)
    .width(Length::Fill)
    .into()
}
