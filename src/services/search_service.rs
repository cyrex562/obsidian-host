use crate::error::{AppError, AppResult};
use crate::models::{SearchMatch, SearchResult};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};
use walkdir::WalkDir;

/// Simple in-memory search index
#[derive(Clone)]
pub struct SearchIndex {
    // vault_id -> (file_path -> content)
    indices: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
}

impl SearchIndex {
    pub fn new() -> Self {
        Self {
            indices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Index all markdown files in a vault
    pub fn index_vault(&self, vault_id: &str, vault_path: &str) -> AppResult<usize> {
        let mut file_count = 0;
        let mut index = HashMap::new();

        for entry in WalkDir::new(vault_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip hidden files and directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }

            // Only index markdown files
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "md" {
                        if let Ok(content) = fs::read_to_string(path) {
                            let relative_path = path
                                .strip_prefix(vault_path)
                                .unwrap_or(path)
                                .to_string_lossy()
                                .to_string();

                            index.insert(relative_path, content);
                            file_count += 1;
                        }
                    }
                }
            }
        }

        let mut indices = self
            .indices
            .write()
            .map_err(|_| AppError::InternalError("Failed to acquire write lock".to_string()))?;
        indices.insert(vault_id.to_string(), index);

        Ok(file_count)
    }

    /// Update a single file in the index
    pub fn update_file(&self, vault_id: &str, file_path: &str, content: String) -> AppResult<()> {
        let mut indices = self
            .indices
            .write()
            .map_err(|_| AppError::InternalError("Failed to acquire write lock".to_string()))?;

        let vault_index = indices
            .entry(vault_id.to_string())
            .or_insert_with(HashMap::new);

        vault_index.insert(file_path.to_string(), content);
        Ok(())
    }

    /// Remove a file from the index
    pub fn remove_file(&self, vault_id: &str, file_path: &str) -> AppResult<()> {
        let mut indices = self
            .indices
            .write()
            .map_err(|_| AppError::InternalError("Failed to acquire write lock".to_string()))?;

        if let Some(vault_index) = indices.get_mut(vault_id) {
            vault_index.remove(file_path);
        }

        Ok(())
    }

    /// Search for a query in a vault
    pub fn search(
        &self,
        vault_id: &str,
        query: &str,
        limit: usize,
    ) -> AppResult<Vec<SearchResult>> {
        let indices = self
            .indices
            .read()
            .map_err(|_| AppError::InternalError("Failed to acquire read lock".to_string()))?;

        let vault_index = indices.get(vault_id).ok_or(AppError::NotFound(format!(
            "Vault index not found: {}",
            vault_id
        )))?;

        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for (file_path, content) in vault_index.iter() {
            let mut matches = Vec::new();
            let mut score = 0.0f32;

            // Search in file name/title
            let file_name = Path::new(file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            if file_name.to_lowercase().contains(&query_lower) {
                score += 10.0;
            }

            // Search in content
            for (line_num, line) in content.lines().enumerate() {
                let line_lower = line.to_lowercase();
                if let Some(pos) = line_lower.find(&query_lower) {
                    matches.push(SearchMatch {
                        line_number: line_num + 1,
                        line_text: line.to_string(),
                        match_start: pos,
                        match_end: pos + query.len(),
                    });
                    score += 1.0;

                    // Limit matches per file
                    if matches.len() >= 10 {
                        break;
                    }
                }
            }

            if !matches.is_empty() || score > 0.0 {
                results.push(SearchResult {
                    path: file_path.clone(),
                    title: file_name.to_string(),
                    matches,
                    score,
                });
            }
        }

        // Sort by score (descending)
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit results
        results.truncate(limit);

        Ok(results)
    }

    /// Remove entire vault index
    pub fn remove_vault(&self, vault_id: &str) -> AppResult<()> {
        let mut indices = self
            .indices
            .write()
            .map_err(|_| AppError::InternalError("Failed to acquire write lock".to_string()))?;

        indices.remove(vault_id);
        Ok(())
    }

    /// Get a random markdown file from the vault
    pub fn get_random_file(&self, vault_id: &str) -> AppResult<Option<String>> {
        let indices = self
            .indices
            .read()
            .map_err(|_| AppError::InternalError("Failed to acquire read lock".to_string()))?;

        if let Some(vault_index) = indices.get(vault_id) {
            if vault_index.is_empty() {
                return Ok(None);
            }

            // Collect keys into a vector to pick a random one
            // Note: This is O(n), but for reasonable vault sizes it's fine.
            // Optimization: Maintain a separate Vec of keys if performance becomes an issue.
            let keys: Vec<&String> = vault_index.keys().collect();

            use rand::seq::IndexedRandom;
            let mut rng = rand::rng();

            if let Some(random_key) = keys.choose(&mut rng) {
                return Ok(Some(random_key.to_string()));
            }
        }

        Ok(None)
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}
