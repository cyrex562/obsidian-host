use crate::error::{AppError, AppResult};
use crate::models::{FileContent, FileNode};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct FileService;

impl FileService {
    /// Get file tree for a vault
    pub fn get_file_tree(vault_path: &str) -> AppResult<Vec<FileNode>> {
        let path = Path::new(vault_path);
        if !path.exists() {
            return Err(AppError::NotFound(format!(
                "Vault path does not exist: {}",
                vault_path
            )));
        }

        let mut root_nodes = Vec::new();
        let entries: Vec<_> = fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .collect();

        for entry in entries {
            let path = entry.path();

            // Skip hidden files and folders (starting with .)
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }

            if let Ok(node) = Self::build_file_node(&path, vault_path) {
                root_nodes.push(node);
            }
        }

        // Sort: directories first, then alphabetically
        root_nodes.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        Ok(root_nodes)
    }

    fn build_file_node(path: &Path, vault_root: &str) -> AppResult<FileNode> {
        let metadata = fs::metadata(path)?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let relative_path = path
            .strip_prefix(vault_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| DateTime::from_timestamp(
                t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                0
            ));

        let is_directory = metadata.is_dir();
        let mut children = None;

        if is_directory {
            let mut child_nodes = Vec::new();
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let child_path = entry.path();

                    // Skip hidden files
                    if let Some(name) = child_path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') {
                            continue;
                        }
                    }

                    if let Ok(child) = Self::build_file_node(&child_path, vault_root) {
                        child_nodes.push(child);
                    }
                }
            }

            // Sort children
            child_nodes.sort_by(|a, b| {
                match (a.is_directory, b.is_directory) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }
            });

            children = Some(child_nodes);
        }

        Ok(FileNode {
            name,
            path: relative_path,
            is_directory,
            children,
            size: if !is_directory { Some(metadata.len()) } else { None },
            modified,
        })
    }

    /// Read file content
    pub fn read_file(vault_path: &str, file_path: &str) -> AppResult<FileContent> {
        let full_path = Self::resolve_path(vault_path, file_path)?;

        if !full_path.exists() {
            return Err(AppError::NotFound(format!("File not found: {}", file_path)));
        }

        if full_path.is_dir() {
            return Err(AppError::InvalidInput("Cannot read a directory".to_string()));
        }

        let content = fs::read_to_string(&full_path)?;
        let metadata = fs::metadata(&full_path)?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| DateTime::from_timestamp(
                t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                0
            ))
            .unwrap_or_else(Utc::now);

        Ok(FileContent {
            path: file_path.to_string(),
            content,
            modified,
        })
    }

    /// Write file content with conflict detection
    pub fn write_file(
        vault_path: &str,
        file_path: &str,
        content: &str,
        last_modified: Option<DateTime<Utc>>,
    ) -> AppResult<FileContent> {
        let full_path = Self::resolve_path(vault_path, file_path)?;

        // Check for conflicts if last_modified is provided
        if full_path.exists() && last_modified.is_some() {
            let metadata = fs::metadata(&full_path)?;
            if let Ok(modified_time) = metadata.modified() {
                let file_modified = DateTime::from_timestamp(
                    modified_time.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .ok_or(AppError::InternalError("Invalid timestamp".to_string()))?
                        .as_secs() as i64,
                    0
                ).ok_or(AppError::InternalError("Invalid timestamp".to_string()))?;

                if let Some(last_mod) = last_modified {
                    // Allow 1 second tolerance for filesystem timestamp precision
                    if file_modified.signed_duration_since(last_mod).num_seconds() > 1 {
                        // Conflict detected - create backup
                        Self::create_conflict_backup(vault_path, file_path)?;
                        return Err(AppError::Conflict(format!(
                            "File was modified externally. Conflict backup created: conflict_{}_{}",
                            file_path,
                            Utc::now().format("%Y%m%d_%H%M%S")
                        )));
                    }
                }
            }
        }

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&full_path, content)?;

        let metadata = fs::metadata(&full_path)?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| DateTime::from_timestamp(
                t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                0
            ))
            .unwrap_or_else(Utc::now);

        Ok(FileContent {
            path: file_path.to_string(),
            content: content.to_string(),
            modified,
        })
    }

    /// Create a conflict backup file
    fn create_conflict_backup(vault_path: &str, file_path: &str) -> AppResult<()> {
        let full_path = Self::resolve_path(vault_path, file_path)?;

        if !full_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&full_path)?;
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");

        let conflict_filename = format!("conflict_{}_{}",
            full_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file"),
            timestamp
        );

        let conflict_path = if let Some(ext) = full_path.extension().and_then(|s| s.to_str()) {
            full_path.with_file_name(format!("{}.{}", conflict_filename, ext))
        } else {
            full_path.with_file_name(conflict_filename)
        };

        fs::write(conflict_path, content)?;
        Ok(())
    }

    /// Create a new file
    pub fn create_file(vault_path: &str, file_path: &str, content: Option<&str>) -> AppResult<FileContent> {
        let full_path = Self::resolve_path(vault_path, file_path)?;

        if full_path.exists() {
            return Err(AppError::Conflict(format!("File already exists: {}", file_path)));
        }

        // Create parent directories
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content_str = content.unwrap_or("");
        fs::write(&full_path, content_str)?;

        let metadata = fs::metadata(&full_path)?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| DateTime::from_timestamp(
                t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                0
            ))
            .unwrap_or_else(Utc::now);

        Ok(FileContent {
            path: file_path.to_string(),
            content: content_str.to_string(),
            modified,
        })
    }

    /// Delete a file
    pub fn delete_file(vault_path: &str, file_path: &str) -> AppResult<()> {
        let full_path = Self::resolve_path(vault_path, file_path)?;

        if !full_path.exists() {
            return Err(AppError::NotFound(format!("File not found: {}", file_path)));
        }

        if full_path.is_dir() {
            fs::remove_dir_all(&full_path)?;
        } else {
            fs::remove_file(&full_path)?;
        }

        Ok(())
    }

    /// Create a directory
    pub fn create_directory(vault_path: &str, dir_path: &str) -> AppResult<()> {
        let full_path = Self::resolve_path(vault_path, dir_path)?;

        if full_path.exists() {
            return Err(AppError::Conflict(format!("Directory already exists: {}", dir_path)));
        }

        fs::create_dir_all(&full_path)?;
        Ok(())
    }

    /// Rename/move a file or directory
    pub fn rename(vault_path: &str, from: &str, to: &str) -> AppResult<()> {
        let from_path = Self::resolve_path(vault_path, from)?;
        let to_path = Self::resolve_path(vault_path, to)?;

        if !from_path.exists() {
            return Err(AppError::NotFound(format!("Source not found: {}", from)));
        }

        if to_path.exists() {
            return Err(AppError::Conflict(format!("Destination already exists: {}", to)));
        }

        // Create parent directory for destination
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::rename(from_path, to_path)?;
        Ok(())
    }

    /// Resolve and validate a path within the vault
    fn resolve_path(vault_path: &str, file_path: &str) -> AppResult<PathBuf> {
        let vault = Path::new(vault_path).canonicalize()
            .map_err(|_| AppError::NotFound(format!("Vault not found: {}", vault_path)))?;

        let full_path = vault.join(file_path);

        // Prevent path traversal attacks
        let canonical = if full_path.exists() {
            full_path.canonicalize()?
        } else {
            // For non-existent paths, validate the parent
            if let Some(parent) = full_path.parent() {
                if parent.exists() {
                    parent.canonicalize()?.join(
                        full_path.file_name()
                            .ok_or(AppError::InvalidInput("Invalid file path".to_string()))?
                    )
                } else {
                    full_path
                }
            } else {
                full_path
            }
        };

        if !canonical.starts_with(&vault) {
            return Err(AppError::InvalidInput(
                "Path is outside vault directory".to_string()
            ));
        }

        Ok(canonical)
    }
}
