use crate::error::{AppError, AppResult};
use crate::models::{FileChangeEvent, FileChangeType};
use chrono::Utc;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, NoCache};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};

pub type ChangeReceiver = mpsc::UnboundedReceiver<FileChangeEvent>;
pub type ChangeSender = mpsc::UnboundedSender<FileChangeEvent>;

pub struct FileWatcher {
    debouncer: Debouncer<RecommendedWatcher, NoCache>,
    vault_paths: Arc<RwLock<HashMap<String, PathBuf>>>,
    change_tx: ChangeSender,
}

impl FileWatcher {
    pub fn new() -> AppResult<(Self, ChangeReceiver)> {
        let (change_tx, change_rx) = mpsc::unbounded_channel();
        let vault_paths = Arc::new(RwLock::new(HashMap::new()));
        let vault_paths_clone = vault_paths.clone();
        let tx_clone = change_tx.clone();

        let debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    for event in events {
                        if let Err(e) =
                            Self::handle_event(event.event, &vault_paths_clone, &tx_clone)
                        {
                            error!("Error handling file event: {}", e);
                        }
                    }
                }
                Err(errors) => {
                    for error in errors {
                        error!("Watch error: {:?}", error);
                    }
                }
            },
        )
        .map_err(|e| AppError::InternalError(format!("Failed to create watcher: {}", e)))?;

        Ok((
            Self {
                debouncer,
                vault_paths,
                change_tx,
            },
            change_rx,
        ))
    }

    pub fn watch_vault(&mut self, vault_id: String, vault_path: PathBuf) -> AppResult<()> {
        info!("Starting to watch vault: {} at {:?}", vault_id, vault_path);

        self.debouncer
            .watch(&vault_path, RecursiveMode::Recursive)
            .map_err(|e| AppError::InternalError(format!("Failed to watch path: {}", e)))?;

        let mut paths = self
            .vault_paths
            .write()
            .map_err(|_| AppError::InternalError("Failed to acquire write lock".to_string()))?;
        paths.insert(vault_id, vault_path);

        Ok(())
    }

    pub fn unwatch_vault(&mut self, vault_id: &str) -> AppResult<()> {
        let mut paths = self
            .vault_paths
            .write()
            .map_err(|_| AppError::InternalError("Failed to acquire write lock".to_string()))?;

        if let Some(vault_path) = paths.remove(vault_id) {
            info!("Stopping watch for vault: {} at {:?}", vault_id, vault_path);

            self.debouncer
                .unwatch(&vault_path)
                .map_err(|e| AppError::InternalError(format!("Failed to unwatch path: {}", e)))?;
        }

        Ok(())
    }

    fn handle_event(
        event: Event,
        vault_paths: &Arc<RwLock<HashMap<String, PathBuf>>>,
        tx: &ChangeSender,
    ) -> AppResult<()> {
        // Skip if no paths in event
        if event.paths.is_empty() {
            return Ok(());
        }

        let paths = vault_paths
            .read()
            .map_err(|_| AppError::InternalError("Failed to acquire read lock".to_string()))?;

        // Find which vault this event belongs to
        for (vault_id, vault_path) in paths.iter() {
            for path in &event.paths {
                if path.starts_with(vault_path) {
                    // Skip hidden files and .obsidian directory
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') {
                            continue;
                        }
                    }

                    let relative_path = path
                        .strip_prefix(vault_path)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .to_string();

                    let event_type = match event.kind {
                        EventKind::Create(_) => FileChangeType::Created,
                        EventKind::Modify(_) => FileChangeType::Modified,
                        EventKind::Remove(_) => FileChangeType::Deleted,
                        _ => continue,
                    };

                    let change_event = FileChangeEvent {
                        vault_id: vault_id.clone(),
                        path: relative_path,
                        event_type,
                        timestamp: Utc::now(),
                    };

                    if let Err(e) = tx.send(change_event) {
                        error!("Failed to send change event: {}", e);
                    }

                    break;
                }
            }
        }

        Ok(())
    }

    pub fn get_sender(&self) -> ChangeSender {
        self.change_tx.clone()
    }
}
