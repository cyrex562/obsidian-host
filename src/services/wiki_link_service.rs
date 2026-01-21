use crate::error::AppResult;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Result of resolving a wiki link
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLink {
    /// The resolved file path relative to vault root
    pub path: String,
    /// Whether the link target exists
    pub exists: bool,
    /// If ambiguous, list of all matching paths
    pub alternatives: Vec<String>,
}

pub struct WikiLinkResolver;

impl WikiLinkResolver {
    /// Resolve a wiki link to an actual file path within the vault
    ///
    /// Obsidian resolution rules:
    /// 1. If the link contains a path separator, treat as relative/absolute path
    /// 2. If not, search for files with matching name (case-insensitive)
    /// 3. `.md` extension is optional and assumed for markdown files
    /// 4. Prefer exact matches over partial matches
    /// 5. If multiple matches, return the "shortest path" match (Obsidian behavior)
    pub fn resolve(vault_path: &str, wiki_link: &str) -> AppResult<ResolvedLink> {
        // Strip fragment identifier (e.g., #header or #^block)
        let (link_target, _fragment) = Self::split_fragment(wiki_link);

        // Decode percent-encoded characters (e.g., %20 -> space)
        let link_target = Self::decode_percent_encoding(&link_target);

        // Check if the link contains a path separator (explicit path)
        if link_target.contains('/') || link_target.contains('\\') {
            return Self::resolve_explicit_path(vault_path, &link_target);
        }

        // Search for matching files in the vault
        Self::resolve_by_name(vault_path, &link_target)
    }

    /// Resolve a wiki link with context of the current file (for relative resolution)
    pub fn resolve_relative(
        vault_path: &str,
        wiki_link: &str,
        current_file: &str,
    ) -> AppResult<ResolvedLink> {
        let (link_target, _fragment) = Self::split_fragment(wiki_link);
        let link_target = Self::decode_percent_encoding(&link_target);

        // If it's an explicit path starting with ./ or ../, resolve relative to current file
        if link_target.starts_with("./") || link_target.starts_with("../") {
            let current_dir = Path::new(current_file)
                .parent()
                .unwrap_or(Path::new(""));

            let resolved = current_dir.join(&link_target);
            let normalized = Self::normalize_path(&resolved);

            return Self::resolve_explicit_path(vault_path, &normalized.to_string_lossy());
        }

        // Otherwise, use standard resolution
        Self::resolve(vault_path, wiki_link)
    }

    /// Split a wiki link into target and fragment (header/block reference)
    fn split_fragment(link: &str) -> (String, Option<String>) {
        if let Some(hash_pos) = link.find('#') {
            let target = link[..hash_pos].to_string();
            let fragment = link[hash_pos + 1..].to_string();
            (target, Some(fragment))
        } else {
            (link.to_string(), None)
        }
    }

    /// Decode percent-encoded characters
    fn decode_percent_encoding(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '%' {
                let hex: String = chars.by_ref().take(2).collect();
                if hex.len() == 2 {
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte as char);
                        continue;
                    }
                }
                // If decoding fails, keep the original %XX
                result.push('%');
                result.push_str(&hex);
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Resolve an explicit path (contains / or \)
    fn resolve_explicit_path(vault_path: &str, link_target: &str) -> AppResult<ResolvedLink> {
        let vault = Path::new(vault_path);

        // Try with .md extension first, then without
        let candidates = vec![
            format!("{}.md", link_target),
            link_target.to_string(),
        ];

        for candidate in &candidates {
            let full_path = vault.join(candidate);
            if full_path.exists() {
                // Verify it's within the vault
                let canonical = full_path.canonicalize()?;
                let vault_canonical = vault.canonicalize()?;

                if canonical.starts_with(&vault_canonical) {
                    return Ok(ResolvedLink {
                        path: candidate.clone(),
                        exists: true,
                        alternatives: vec![],
                    });
                }
            }
        }

        // File doesn't exist, return the path with .md extension as default
        let default_path = if link_target.ends_with(".md") {
            link_target.to_string()
        } else {
            format!("{}.md", link_target)
        };

        Ok(ResolvedLink {
            path: default_path,
            exists: false,
            alternatives: vec![],
        })
    }

    /// Resolve by searching for files with matching name
    fn resolve_by_name(vault_path: &str, name: &str) -> AppResult<ResolvedLink> {
        let vault = Path::new(vault_path);

        // Normalize the search name (handle .md extension)
        let name_lower = name.to_lowercase();
        let name_stem_lower = if name_lower.ends_with(".md") {
            name_lower[..name_lower.len() - 3].to_string()
        } else {
            name_lower.clone()
        };
        let name_with_md_lower = format!("{}.md", name_stem_lower);

        // Collect all matching files
        let mut matches: Vec<(String, usize)> = Vec::new(); // (path, depth)

        for entry in WalkDir::new(vault)
            .follow_links(false)
            .max_depth(20)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Get path relative to vault for hidden file check
            let relative = path.strip_prefix(vault).unwrap_or(path);

            // Skip hidden files/directories (only check relative path components)
            if relative
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            {
                continue;
            }

            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    let file_name_lower = file_name.to_lowercase();
                    let stem_lower = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();

                    // Check for match (case-insensitive)
                    // Match if:
                    // 1. Full filename matches (with or without .md)
                    // 2. File stem matches the search term
                    let is_match = file_name_lower == name_lower
                        || file_name_lower == name_with_md_lower
                        || file_name_lower == name_stem_lower
                        || stem_lower == name_stem_lower;

                    if is_match {
                        let relative = path
                            .strip_prefix(vault)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();

                        let depth = relative.matches('/').count() + relative.matches('\\').count();
                        matches.push((relative, depth));
                    }
                }
            }
        }

        if matches.is_empty() {
            // No match found - return suggested path
            let default_path = if name.ends_with(".md") {
                name.to_string()
            } else {
                format!("{}.md", name)
            };

            return Ok(ResolvedLink {
                path: default_path,
                exists: false,
                alternatives: vec![],
            });
        }

        // Sort by depth (shortest path first), then alphabetically
        matches.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

        let primary = matches[0].0.clone();
        let alternatives: Vec<String> = if matches.len() > 1 {
            matches.iter().skip(1).map(|(p, _)| p.clone()).collect()
        } else {
            vec![]
        };

        Ok(ResolvedLink {
            path: primary,
            exists: true,
            alternatives,
        })
    }

    /// Normalize a path (remove . and .. components)
    fn normalize_path(path: &Path) -> PathBuf {
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    components.pop();
                }
                std::path::Component::CurDir => {}
                _ => {
                    components.push(component);
                }
            }
        }

        components.iter().collect()
    }

    /// Build an index of all files in the vault for faster lookups
    pub fn build_file_index(vault_path: &str) -> AppResult<FileIndex> {
        let vault = Path::new(vault_path);
        let mut index = FileIndex::new();

        for entry in WalkDir::new(vault)
            .follow_links(false)
            .max_depth(20)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Get path relative to vault for hidden file check
            let relative = path.strip_prefix(vault).unwrap_or(path);

            // Skip hidden files/directories (only check relative path components)
            if relative
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            {
                continue;
            }

            if path.is_file() {
                let relative_str = relative.to_string_lossy().to_string();

                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(file_name);

                    let depth = relative_str.matches('/').count() + relative_str.matches('\\').count();

                    index.add_file(stem, file_name, &relative_str, depth);
                }
            }
        }

        Ok(index)
    }
}

/// Pre-built index of files for faster wiki link resolution
#[derive(Debug, Default)]
pub struct FileIndex {
    /// Map from lowercase file stem to list of (relative_path, depth)
    by_stem: HashMap<String, Vec<(String, usize)>>,
    /// Map from lowercase full filename to list of (relative_path, depth)
    by_name: HashMap<String, Vec<(String, usize)>>,
}

impl FileIndex {
    pub fn new() -> Self {
        Self::default()
    }

    fn add_file(&mut self, stem: &str, name: &str, path: &str, depth: usize) {
        let stem_lower = stem.to_lowercase();
        let name_lower = name.to_lowercase();

        self.by_stem
            .entry(stem_lower)
            .or_default()
            .push((path.to_string(), depth));

        self.by_name
            .entry(name_lower)
            .or_default()
            .push((path.to_string(), depth));
    }

    /// Resolve a wiki link using the pre-built index
    pub fn resolve(&self, link: &str) -> ResolvedLink {
        let (link_target, _fragment) = WikiLinkResolver::split_fragment(link);
        let link_target = WikiLinkResolver::decode_percent_encoding(&link_target);
        let link_lower = link_target.to_lowercase();

        // Try exact filename match first
        let matches = self
            .by_name
            .get(&link_lower)
            .or_else(|| self.by_name.get(&format!("{}.md", link_lower)))
            .or_else(|| self.by_stem.get(&link_lower));

        match matches {
            Some(paths) if !paths.is_empty() => {
                let mut sorted = paths.clone();
                sorted.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

                let primary = sorted[0].0.clone();
                let alternatives: Vec<String> = if sorted.len() > 1 {
                    sorted.iter().skip(1).map(|(p, _)| p.clone()).collect()
                } else {
                    vec![]
                };

                ResolvedLink {
                    path: primary,
                    exists: true,
                    alternatives,
                }
            }
            _ => {
                let default_path = if link_target.ends_with(".md") {
                    link_target.to_string()
                } else {
                    format!("{}.md", link_target)
                };

                ResolvedLink {
                    path: default_path,
                    exists: false,
                    alternatives: vec![],
                }
            }
        }
    }

    /// Get total number of indexed files
    pub fn file_count(&self) -> usize {
        self.by_name.values().map(|v| v.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_vault() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let vault = temp_dir.path();

        // Create test files
        fs::write(vault.join("Note.md"), "# Note").unwrap();
        fs::write(vault.join("Another Note.md"), "# Another Note").unwrap();

        // Create subdirectory with files
        fs::create_dir(vault.join("folder")).unwrap();
        fs::write(vault.join("folder/Note.md"), "# Folder Note").unwrap();
        fs::write(vault.join("folder/SubNote.md"), "# SubNote").unwrap();

        // Create nested subdirectory
        fs::create_dir(vault.join("folder/nested")).unwrap();
        fs::write(vault.join("folder/nested/Deep.md"), "# Deep Note").unwrap();

        temp_dir
    }

    #[test]
    fn test_resolve_simple_link() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let result = WikiLinkResolver::resolve(vault_path, "Note").unwrap();
        assert!(result.exists);
        assert!(result.path == "Note.md" || result.path == "folder/Note.md");
    }

    #[test]
    fn test_resolve_with_extension() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let result = WikiLinkResolver::resolve(vault_path, "Note.md").unwrap();
        assert!(result.exists);
    }

    #[test]
    fn test_resolve_explicit_path() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let result = WikiLinkResolver::resolve(vault_path, "folder/SubNote").unwrap();
        assert!(result.exists);
        assert_eq!(result.path, "folder/SubNote.md");
    }

    #[test]
    fn test_resolve_nonexistent() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let result = WikiLinkResolver::resolve(vault_path, "NonExistent").unwrap();
        assert!(!result.exists);
        assert_eq!(result.path, "NonExistent.md");
    }

    #[test]
    fn test_resolve_with_spaces() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let result = WikiLinkResolver::resolve(vault_path, "Another Note").unwrap();
        assert!(result.exists);
        assert_eq!(result.path, "Another Note.md");
    }

    #[test]
    fn test_resolve_percent_encoded() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let result = WikiLinkResolver::resolve(vault_path, "Another%20Note").unwrap();
        assert!(result.exists);
        assert_eq!(result.path, "Another Note.md");
    }

    #[test]
    fn test_resolve_with_fragment() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let result = WikiLinkResolver::resolve(vault_path, "Note#header").unwrap();
        assert!(result.exists);
    }

    #[test]
    fn test_ambiguous_link_returns_alternatives() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let result = WikiLinkResolver::resolve(vault_path, "Note").unwrap();
        // Should find both Note.md and folder/Note.md
        // Primary should be the one with shortest path (Note.md)
        assert!(result.exists);
        assert_eq!(result.path, "Note.md");
        assert!(result.alternatives.contains(&"folder/Note.md".to_string()));
    }

    #[test]
    fn test_file_index() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let index = WikiLinkResolver::build_file_index(vault_path).unwrap();
        assert!(index.file_count() >= 5);

        let result = index.resolve("Note");
        assert!(result.exists);
    }

    #[test]
    fn test_case_insensitive_resolution() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();

        let result = WikiLinkResolver::resolve(vault_path, "note").unwrap();
        assert!(result.exists);

        let result2 = WikiLinkResolver::resolve(vault_path, "NOTE").unwrap();
        assert!(result2.exists);
    }
}
