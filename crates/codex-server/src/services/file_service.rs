use crate::error::{AppError, AppResult};
use crate::models::{FileContent, FileNode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Represents a file that has been moved to the vault's `.trash/` folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashItem {
    /// The timestamped filename inside `.trash/` (e.g. `20240101_120000.000_notes.md`).
    pub trash_name: String,
    /// The original vault-relative path before the file was trashed.
    pub original_path: String,
    /// RFC-3339 timestamp of when the file was trashed.
    pub trashed_at: String,
}

/// Convert a `SystemTime` to `DateTime<Utc>` preserving sub-second precision.
/// Using `subsec_nanos()` ensures that two writes in the same second still
/// produce distinct ETags as long as the filesystem has nanosecond mtime.
fn system_time_to_datetime(t: std::time::SystemTime) -> Option<DateTime<Utc>> {
    let dur = t.duration_since(std::time::UNIX_EPOCH).ok()?;
    DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
}

pub struct FileService;

#[derive(Debug, PartialEq)]
pub enum RenameStrategy {
    Fail,
    Overwrite,
    AutoRename, // e.g., file.txt -> file (1).txt
}

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
        let entries: Vec<_> = fs::read_dir(path)?.filter_map(|e| e.ok()).collect();

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
        root_nodes.sort_by(|a, b| match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
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

        let modified = metadata.modified().ok().and_then(system_time_to_datetime);

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
            child_nodes.sort_by(|a, b| match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });

            children = Some(child_nodes);
        }

        Ok(FileNode {
            name,
            path: relative_path,
            is_directory,
            children,
            size: if !is_directory {
                Some(metadata.len())
            } else {
                None
            },
            modified,
        })
    }

    /// Read file content
    pub fn read_file(vault_path: &str, file_path: &str) -> AppResult<FileContent> {
        debug!(vault_path = %vault_path, file_path = %file_path, "Reading file");
        let full_path = Self::resolve_path(vault_path, file_path)?;

        if !full_path.exists() {
            return Err(AppError::NotFound(format!("File not found: {}", file_path)));
        }

        if full_path.is_dir() {
            return Err(AppError::InvalidInput(
                "Cannot read a directory".to_string(),
            ));
        }

        let raw_content = fs::read_to_string(&full_path)?;
        let metadata = fs::metadata(&full_path)?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(system_time_to_datetime)
            .unwrap_or_else(Utc::now);
        let (frontmatter, content) = if file_path.ends_with(".md") {
            crate::services::frontmatter_service::parse_frontmatter(&raw_content)?
        } else {
            (None, raw_content)
        };

        Ok(FileContent {
            path: file_path.to_string(),
            content,
            modified,
            frontmatter,
        })
    }

    /// Read raw file content (for binary files like images, PDFs, etc.)
    pub fn read_raw_file(vault_path: &str, file_path: &str) -> AppResult<Vec<u8>> {
        let full_path = Self::resolve_path(vault_path, file_path)?;

        if !full_path.exists() {
            return Err(AppError::NotFound(format!("File not found: {}", file_path)));
        }

        if full_path.is_dir() {
            return Err(AppError::InvalidInput(
                "Cannot read a directory".to_string(),
            ));
        }

        let content = fs::read(&full_path)?;
        Ok(content)
    }

    /// Write file content with conflict detection
    pub fn write_file(
        vault_path: &str,
        file_path: &str,
        content: &str,
        last_modified: Option<DateTime<Utc>>,
        frontmatter: Option<&serde_json::Value>,
    ) -> AppResult<FileContent> {
        debug!(vault_path = %vault_path, file_path = %file_path, size = content.len(), "Writing file");
        let full_path = Self::resolve_path(vault_path, file_path)?;

        // Check for conflicts if last_modified is provided
        if full_path.exists() && last_modified.is_some() {
            let metadata = fs::metadata(&full_path)?;
            if let Ok(modified_time) = metadata.modified() {
                let file_modified = system_time_to_datetime(modified_time)
                    .ok_or(AppError::InternalError("Invalid timestamp".to_string()))?;

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

        // Serialize frontmatter with content for markdown files
        let final_content = if file_path.ends_with(".md") {
            crate::services::frontmatter_service::serialize_frontmatter(frontmatter, content)?
        } else {
            content.to_string()
        };

        fs::write(&full_path, &final_content)?;

        let metadata = fs::metadata(&full_path)?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(system_time_to_datetime)
            .unwrap_or_else(Utc::now);

        Ok(FileContent {
            path: file_path.to_string(),
            content: content.to_string(),
            modified,
            frontmatter: frontmatter.cloned(),
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

        let conflict_filename = format!(
            "conflict_{}_{}",
            full_path
                .file_stem()
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
    pub fn create_file(
        vault_path: &str,
        file_path: &str,
        content: Option<&str>,
    ) -> AppResult<FileContent> {
        debug!(vault_path = %vault_path, file_path = %file_path, "Creating file");
        let full_path = Self::resolve_path(vault_path, file_path)?;

        if full_path.exists() {
            return Err(AppError::Conflict(format!(
                "File already exists: {}",
                file_path
            )));
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
            .and_then(system_time_to_datetime)
            .unwrap_or_else(Utc::now);

        Ok(FileContent {
            path: file_path.to_string(),
            content: content_str.to_string(),
            modified,
            frontmatter: None,
        })
    }

    /// Delete a file (move to trash)
    pub fn delete_file(vault_path: &str, file_path: &str) -> AppResult<()> {
        info!(vault_path = %vault_path, file_path = %file_path, "Deleting file (moving to trash)");
        Self::move_to_trash(vault_path, file_path)
    }

    /// Move file to .trash folder, writing a sidecar `.meta.json` with the original path
    /// so the file can later be restored.
    pub fn move_to_trash(vault_path: &str, file_path: &str) -> AppResult<()> {
        let full_path = Self::resolve_path(vault_path, file_path)?;

        if !full_path.exists() {
            return Err(AppError::NotFound(format!("File not found: {}", file_path)));
        }

        let trash_dir = Path::new(vault_path).join(".trash");
        if !trash_dir.exists() {
            fs::create_dir(&trash_dir)?;
        }

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S%.3f");
        let file_name = full_path.file_name().unwrap_or_default().to_string_lossy();
        let trash_name = format!("{timestamp}_{file_name}");

        let dest_path = trash_dir.join(&trash_name);
        fs::rename(&full_path, &dest_path)?;

        // Write sidecar so we can restore the original path later
        let meta = serde_json::json!({
            "original_path": file_path,
            "trashed_at": Utc::now().to_rfc3339(),
        });
        let meta_path = trash_dir.join(format!("{trash_name}.meta.json"));
        fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)?;

        Ok(())
    }

    /// List items currently in the vault's .trash folder.
    ///
    /// Returns `(trash_name, original_path, trashed_at)` for each trashed file.
    pub fn list_trash(vault_path: &str) -> AppResult<Vec<TrashItem>> {
        let trash_dir = Path::new(vault_path).join(".trash");
        if !trash_dir.exists() {
            return Ok(vec![]);
        }

        let mut items = Vec::new();
        for entry in fs::read_dir(&trash_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            // Only process meta sidecars; skip actual content files
            if !name.ends_with(".meta.json") {
                continue;
            }
            let trash_name = name.trim_end_matches(".meta.json").to_string();
            let meta_bytes = fs::read(entry.path())?;
            let meta: serde_json::Value = serde_json::from_slice(&meta_bytes)
                .unwrap_or(serde_json::Value::Null);
            items.push(TrashItem {
                trash_name,
                original_path: meta["original_path"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                trashed_at: meta["trashed_at"].as_str().unwrap_or("").to_string(),
            });
        }
        items.sort_by(|a, b| b.trashed_at.cmp(&a.trashed_at));
        Ok(items)
    }

    /// Restore a file from trash to its original location.
    ///
    /// `trash_name` is the timestamped filename in `.trash/` (as returned by `list_trash`).
    pub fn restore_file(vault_path: &str, trash_name: &str) -> AppResult<()> {
        // Validate that trash_name contains no path separators (basic traversal guard)
        if trash_name.contains('/') || trash_name.contains('\\') || trash_name.contains("..") {
            return Err(AppError::InvalidInput(
                "Invalid trash_name: must be a plain file name with no path components".to_string(),
            ));
        }

        let trash_dir = Path::new(vault_path).join(".trash");
        let trashed_file = trash_dir.join(trash_name);
        let meta_file = trash_dir.join(format!("{trash_name}.meta.json"));

        if !trashed_file.exists() {
            return Err(AppError::NotFound(format!(
                "Trashed file not found: {trash_name}"
            )));
        }
        if !meta_file.exists() {
            return Err(AppError::NotFound(format!(
                "Trash metadata not found for: {trash_name}. File may have been trashed before restore support was added."
            )));
        }

        let meta_bytes = fs::read(&meta_file)?;
        let meta: serde_json::Value = serde_json::from_slice(&meta_bytes)
            .map_err(|e| AppError::InternalError(format!("Corrupt trash metadata: {e}")))?;
        let original_path = meta["original_path"]
            .as_str()
            .ok_or_else(|| AppError::InternalError("Missing original_path in trash metadata".to_string()))?;

        let restore_path = Self::resolve_path(vault_path, original_path)?;

        // Ensure the parent directory exists
        if let Some(parent) = restore_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if restore_path.exists() {
            return Err(AppError::Conflict(format!(
                "Cannot restore: a file already exists at '{original_path}'"
            )));
        }

        fs::rename(&trashed_file, &restore_path)?;
        fs::remove_file(&meta_file)?;

        Ok(())
    }

    /// Permanently delete a file from trash (no recovery possible).
    pub fn delete_from_trash(vault_path: &str, trash_name: &str) -> AppResult<()> {
        if trash_name.contains('/') || trash_name.contains('\\') || trash_name.contains("..") {
            return Err(AppError::InvalidInput(
                "Invalid trash_name: must be a plain file name with no path components".to_string(),
            ));
        }

        let trash_dir = Path::new(vault_path).join(".trash");
        let trashed_file = trash_dir.join(trash_name);
        let meta_file = trash_dir.join(format!("{trash_name}.meta.json"));

        if !trashed_file.exists() {
            return Err(AppError::NotFound(format!(
                "Trashed file not found: {trash_name}"
            )));
        }

        fs::remove_file(&trashed_file)?;
        let _ = fs::remove_file(&meta_file); // best-effort; meta may be missing for old items
        Ok(())
    }

    /// Create a directory
    pub fn create_directory(vault_path: &str, dir_path: &str) -> AppResult<()> {
        let full_path = Self::resolve_path(vault_path, dir_path)?;

        if full_path.exists() {
            return Err(AppError::Conflict(format!(
                "Directory already exists: {}",
                dir_path
            )));
        }

        fs::create_dir_all(&full_path)?;
        Ok(())
    }

    /// Rename/move a file or directory with conflict handling strategy
    pub fn rename(
        vault_path: &str,
        from: &str,
        to: &str,
        strategy: RenameStrategy,
    ) -> AppResult<String> {
        let from_path = Self::resolve_path(vault_path, from)?;
        let mut to_path = Self::resolve_path(vault_path, to)?;
        let mut final_to = to.to_string();

        if !from_path.exists() {
            return Err(AppError::NotFound(format!("Source not found: {}", from)));
        }

        if from_path == to_path {
            return Ok(final_to);
        }

        if to_path.exists() {
            match strategy {
                RenameStrategy::Fail => {
                    return Err(AppError::Conflict(format!(
                        "Destination already exists: {}",
                        to
                    )));
                }
                RenameStrategy::Overwrite => {
                    // If overwrite, we might need to remove destination first if it's a directory
                    // or if type mismatch (file vs dir)
                    if to_path.is_dir() {
                        if from_path.is_file() {
                            return Err(AppError::Conflict(
                                "Cannot overwrite directory with file".to_string(),
                            ));
                        }
                        // If both are directories, overwrite usually means merge or replace?
                        // Replace is dangerous. Let's stick to file overwrite or empty dir overwrite.
                        fs::remove_dir_all(&to_path)?;
                    } else {
                        // Destination is file
                        if from_path.is_dir() {
                            return Err(AppError::Conflict(
                                "Cannot overwrite file with directory".to_string(),
                            ));
                        }
                        // File overwriting file - remove dest first to be clean or just rename over
                        // fs::rename overwrites on *nix, but windows might have issues?
                        // Rust std::fs::rename says "This will replace the destination path if it already exists" on Unix,
                        // but on Windows "It will fail if a file or directory at the destination path already exists".
                        // So we MUST delete destination on Windows (and safe on Unix too).
                        if to_path.exists() {
                            fs::remove_file(&to_path)?;
                        }
                    }
                }
                RenameStrategy::AutoRename => {
                    // Generate new name: name (1).ext, name (2).ext
                    let mut counter = 1;
                    let stem = to_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("file");
                    let ext = to_path.extension().and_then(|s| s.to_str());
                    let parent = to_path
                        .parent()
                        .ok_or(AppError::InternalError("Invalid path".to_string()))?;

                    loop {
                        let new_name = if let Some(extension) = ext {
                            format!("{} ({}) .{}", stem, counter, extension)
                        } else {
                            format!("{} ({})", stem, counter)
                        };

                        let new_path = parent.join(&new_name);
                        if !new_path.exists() {
                            to_path = new_path;
                            // Update final_to relative path
                            // We need to reconstruct relative path manually or extract from params
                            // Simplest is to take 'to' parent and join new name
                            let to_parent = Path::new(to).parent().unwrap_or(Path::new(""));
                            final_to = to_parent.join(&new_name).to_string_lossy().to_string();
                            break;
                        }
                        counter += 1;
                        if counter > 1000 {
                            return Err(AppError::Conflict(
                                "Could not find unique name".to_string(),
                            ));
                        }
                    }
                }
            }
        }

        // Create parent directory for destination
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::rename(from_path, to_path)?;
        Ok(final_to)
    }

    /// Resolve and validate a path within the vault
    pub fn resolve_path(vault_path: &str, file_path: &str) -> AppResult<PathBuf> {
        let vault = Path::new(vault_path)
            .canonicalize()
            .map_err(|_| AppError::NotFound(format!("Vault not found: {}", vault_path)))?;

        // 1. Basic check for absolute paths
        let path_to_check = Path::new(file_path);
        if path_to_check.is_absolute() {
            return Err(AppError::InvalidInput(
                "Absolute paths are not allowed".to_string(),
            ));
        }

        // 2. Component check to reject traversal attempts
        for component in path_to_check.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(AppError::InvalidInput(
                        "Directory traversal (..) is not allowed".to_string(),
                    ));
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    return Err(AppError::InvalidInput(
                        "Root or Prefix components are not allowed".to_string(),
                    ));
                }
                _ => {}
            }
        }

        // 3. Construct full path
        let full_path = vault.join(file_path);

        // 4. Final canonicalization check existence (if it exists)
        // If it exists, canonicalize and ensure it's still under vault (symlink protection)
        if full_path.exists() {
            let canonical = full_path.canonicalize()?;
            if !canonical.starts_with(&vault) {
                return Err(AppError::InvalidInput(
                    "Path resolves outside vault directory".to_string(),
                ));
            }
            Ok(canonical)
        } else {
            // If it doesn't exist, we trust the component check + join
            // We can also canonicalize parent if it exists to be sure
            if let Some(parent) = full_path.parent() {
                if parent.exists() {
                    let canonical_parent = parent.canonicalize()?;
                    if !canonical_parent.starts_with(&vault) {
                        return Err(AppError::InvalidInput(
                            "Parent path resolves outside vault directory".to_string(),
                        ));
                    }
                }
            }
            Ok(full_path)
        }
    }

    // ── Upload-session helpers ────────────────────────────────────────────

    /// Creates the temp directory and empty file for a chunked upload session.
    pub fn create_upload_session_temp(vault_path: &str, session_id: &str) -> AppResult<()> {
        let upload_dir = Path::new(vault_path).join(".obsidian").join("uploads");
        std::fs::create_dir_all(&upload_dir)?;
        let temp_file_path = upload_dir.join(session_id);
        std::fs::File::create(temp_file_path)?;
        Ok(())
    }

    /// Appends a chunk of bytes to an in-progress upload session.
    /// Returns the new total byte size.
    pub fn append_upload_chunk(vault_path: &str, session_id: &str, bytes: &[u8]) -> AppResult<u64> {
        use std::io::Write;
        use std::fs::OpenOptions;
        let temp_file_path = upload_temp_file_path(vault_path, session_id);
        if !temp_file_path.exists() {
            return Err(AppError::NotFound("Upload session not found".to_string()));
        }
        let mut file = OpenOptions::new().append(true).open(&temp_file_path)?;
        file.write_all(bytes)?;
        Ok(file.metadata()?.len())
    }

    /// Returns the current byte size of an upload session temp file.
    pub fn get_upload_session_size(vault_path: &str, session_id: &str) -> AppResult<u64> {
        let temp_file_path = upload_temp_file_path(vault_path, session_id);
        if !temp_file_path.exists() {
            return Err(AppError::NotFound("Upload session not found".to_string()));
        }
        Ok(std::fs::metadata(temp_file_path)?.len())
    }

    /// Moves the upload session temp file to its final destination.
    /// Returns the vault-relative final path.
    pub fn finalize_upload_session(
        vault_path: &str,
        session_id: &str,
        target_dir: &str,
        filename: &str,
    ) -> AppResult<String> {
        validate_upload_filename(filename)?;

        let temp_file_path = upload_temp_file_path(vault_path, session_id);
        if !temp_file_path.exists() {
            return Err(AppError::NotFound("Upload session not found".to_string()));
        }

        let safe_target_dir = if target_dir.is_empty() {
            Path::new(vault_path).to_path_buf()
        } else {
            FileService::resolve_path(vault_path, target_dir)?
        };

        if !safe_target_dir.exists() {
            std::fs::create_dir_all(&safe_target_dir)?;
        } else if !safe_target_dir.is_dir() {
            return Err(AppError::InvalidInput(
                "Target path is not a directory".to_string(),
            ));
        }

        let final_path = safe_target_dir.join(filename);
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if std::fs::rename(&temp_file_path, &final_path).is_err() {
            std::fs::copy(&temp_file_path, &final_path)?;
            std::fs::remove_file(&temp_file_path)?;
        }

        let relative = final_path
            .strip_prefix(vault_path)
            .unwrap_or(&final_path)
            .to_string_lossy()
            .to_string();

        Ok(relative)
    }

    /// Removes the upload session temp file (on abort or after finalize).
    pub fn delete_upload_session_temp(vault_path: &str, session_id: &str) -> AppResult<()> {
        let temp_file_path = upload_temp_file_path(vault_path, session_id);
        if temp_file_path.exists() {
            std::fs::remove_file(temp_file_path)?;
        }
        Ok(())
    }

    /// Walk the vault and return (vault-relative-path, content) for every .md file.
    pub fn list_markdown_files(vault_path: &str) -> AppResult<Vec<(String, String)>> {
        use walkdir::WalkDir;
        let mut files = Vec::new();
        for entry in WalkDir::new(vault_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let rel = path
                        .strip_prefix(vault_path)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .to_string();
                    files.push((rel, content));
                }
            }
        }
        Ok(files)
    }
}

fn upload_temp_file_path(vault_path: &str, session_id: &str) -> std::path::PathBuf {
    Path::new(vault_path)
        .join(".obsidian")
        .join("uploads")
        .join(session_id)
}

fn validate_upload_filename(filename: &str) -> AppResult<()> {
    let file_name_path = Path::new(filename);
    let mut components = file_name_path.components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => Ok(()),
        _ => Err(AppError::InvalidInput(
            "Invalid upload filename".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_path_security() {
        let temp = TempDir::new().unwrap();
        let vault_path = temp.path().to_str().unwrap();

        // 1. Valid path
        let path = FileService::resolve_path(vault_path, "note.md");
        assert!(path.is_ok());
        assert!(path.unwrap().ends_with("note.md"));

        // 2. Traversal attempt
        let traversal = FileService::resolve_path(vault_path, "../outside.txt");
        assert!(traversal.is_err());
        assert_eq!(
            traversal.unwrap_err().to_string(),
            "Invalid input: Directory traversal (..) is not allowed"
        );

        // 3. Nested traversal attempt
        let nested_traversal = FileService::resolve_path(vault_path, "folder/../../outside.txt");
        assert!(nested_traversal.is_err());

        // 4. Root dir attempt
        let root_attempt = FileService::resolve_path(vault_path, "/etc/passwd");
        assert!(root_attempt.is_err()); // Either Absolute or RootComponent check catches this

        // 5. Existing file check
        std::fs::write(temp.path().join("existing.md"), "test").unwrap();
        let existing = FileService::resolve_path(vault_path, "existing.md");
        assert!(existing.is_ok());
    }
}
