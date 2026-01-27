use crate::error::{AppError, AppResult};
use crate::models::{FileContent, FileNode};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

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

        let modified = metadata.modified().ok().and_then(|t| {
            DateTime::from_timestamp(
                t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                0,
            )
        });

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
            .and_then(|t| {
                DateTime::from_timestamp(
                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            })
            .unwrap_or_else(Utc::now);

        // Parse frontmatter for markdown files
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
                let file_modified = DateTime::from_timestamp(
                    modified_time
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .ok_or(AppError::InternalError("Invalid timestamp".to_string()))?
                        .as_secs() as i64,
                    0,
                )
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
            .and_then(|t| {
                DateTime::from_timestamp(
                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            })
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
            .and_then(|t| {
                DateTime::from_timestamp(
                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            })
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

    /// Move file to .trash folder
    pub fn move_to_trash(vault_path: &str, file_path: &str) -> AppResult<()> {
        let full_path = Self::resolve_path(vault_path, file_path)?;

        if !full_path.exists() {
            return Err(AppError::NotFound(format!("File not found: {}", file_path)));
        }

        let trash_dir = Path::new(vault_path).join(".trash");
        if !trash_dir.exists() {
            fs::create_dir(&trash_dir)?;
        }

        // Preserve original structure in trash or flat structure?
        // Flat structure with timestamp is easier to implement for simple restoration logic
        // But for true restore, we need original path.
        // Let's implement flat structure with encoded original path for now

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let file_name = full_path.file_name().unwrap_or_default().to_string_lossy();
        let trash_name = format!("{}_{}", timestamp, file_name);

        // We could store metadata about original path in a sidecar file or DB,
        // but for now let's just move it.
        // NOTE: This simple implementation doesn't support full "restore" to original location
        // without more state.
        //
        // Improvement: Move to .trash/original_path structure

        let dest_path = trash_dir.join(&trash_name);

        fs::rename(&full_path, &dest_path)?;

        Ok(())
    }

    /// Restore file from trash (Placeholder - requires ID/Path lookup)
    pub fn restore_file(_vault_path: &str, _trash_id: &str) -> AppResult<()> {
        // Implementation would require tracking original paths
        Err(AppError::InternalError(
            "Restore not fully implemented yet".to_string(),
        ))
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
