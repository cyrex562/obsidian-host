use crate::error::{AppError, AppResult};
use crate::models::{SearchMatch, SearchResult};
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_vault() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let vault = temp_dir.path();

        // Create test markdown files
        fs::write(
            vault.join("Note.md"),
            "# My Note\n\nThis is a test note about Rust programming.\nIt contains multiple lines.",
        )
        .unwrap();

        fs::write(
            vault.join("Another.md"),
            "# Another Note\n\nThis note talks about Python and JavaScript.\nRust is also mentioned here.",
        )
        .unwrap();

        fs::write(
            vault.join("CaseTest.md"),
            "# Case Sensitivity\n\nRUST rust Rust RuSt are all the same.",
        )
        .unwrap();

        // Create subdirectory with files
        fs::create_dir(vault.join("folder")).unwrap();
        fs::write(
            vault.join("folder/Nested.md"),
            "# Nested Note\n\nThis is a nested file about rust programming.",
        )
        .unwrap();

        // Create a non-markdown file (should be ignored)
        fs::write(vault.join("readme.txt"), "This is a text file.").unwrap();

        // Create hidden file (should be ignored)
        fs::write(vault.join(".hidden.md"), "# Hidden\n\nThis should be ignored.").unwrap();

        temp_dir
    }

    #[test]
    fn test_index_vault() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();

        let count = index.index_vault("test-vault", vault_path).unwrap();

        // Should index 4 markdown files (Note.md, Another.md, CaseTest.md, folder/Nested.md)
        // Should NOT index readme.txt or .hidden.md
        assert_eq!(count, 4);
    }

    #[test]
    fn test_basic_search() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "rust", 10).unwrap();

        // Should find matches in Note.md, Another.md, CaseTest.md, and folder/Nested.md
        assert!(!results.is_empty());
        assert!(results.len() >= 3); // At least 3 files mention "rust"
    }

    #[test]
    fn test_case_insensitive_search() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        // Search with different cases should return same results
        let results_lower = index.search("test-vault", "rust", 10).unwrap();
        let results_upper = index.search("test-vault", "RUST", 10).unwrap();
        let results_mixed = index.search("test-vault", "RuSt", 10).unwrap();

        assert_eq!(results_lower.len(), results_upper.len());
        assert_eq!(results_lower.len(), results_mixed.len());
    }

    #[test]
    fn test_filename_match_higher_score() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "note", 10).unwrap();

        // Files with "Note" in the filename should have higher scores
        assert!(!results.is_empty());

        // The first result should be a file with "Note" in the name
        // (since filename match adds 10 points)
        let first = &results[0];
        assert!(
            first.title.to_lowercase().contains("note"),
            "Expected first result to have 'note' in filename, got: {}",
            first.title
        );
    }

    #[test]
    fn test_multiple_matches_in_file() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        // CaseTest.md has multiple "rust" matches on one line
        let results = index.search("test-vault", "rust", 10).unwrap();

        // Find CaseTest result
        let case_test = results.iter().find(|r| r.path.contains("CaseTest"));
        assert!(case_test.is_some());
    }

    #[test]
    fn test_match_positions() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "programming", 10).unwrap();

        // Should find "programming" in content
        assert!(!results.is_empty());

        let result = &results[0];
        assert!(!result.matches.is_empty());

        let first_match = &result.matches[0];
        assert!(first_match.line_text.to_lowercase().contains("programming"));
        assert!(first_match.match_start < first_match.match_end);
    }

    #[test]
    fn test_empty_query() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "", 10).unwrap();

        // Empty query matches everything (empty string is contained in all strings)
        // This is current behavior - might want to handle differently
        assert!(!results.is_empty());
    }

    #[test]
    fn test_no_matches() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "xyznonexistent123", 10).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn test_result_limit() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        // Search for something that matches multiple files but limit to 2
        let results = index.search("test-vault", "note", 2).unwrap();

        assert!(results.len() <= 2);
    }

    #[test]
    fn test_results_sorted_by_score() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "rust", 10).unwrap();

        // Verify results are sorted by score descending
        for i in 1..results.len() {
            assert!(
                results[i - 1].score >= results[i].score,
                "Results not sorted by score: {} < {}",
                results[i - 1].score,
                results[i].score
            );
        }
    }

    #[test]
    fn test_update_file() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        // Initially no "uniqueword" matches
        let results_before = index.search("test-vault", "uniqueword", 10).unwrap();
        assert!(results_before.is_empty());

        // Update a file with new content
        index
            .update_file(
                "test-vault",
                "Note.md",
                "# Updated Note\n\nThis contains uniqueword now.".to_string(),
            )
            .unwrap();

        // Now should find the match
        let results_after = index.search("test-vault", "uniqueword", 10).unwrap();
        assert_eq!(results_after.len(), 1);
        assert_eq!(results_after[0].path, "Note.md");
    }

    #[test]
    fn test_remove_file() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        // Search for Python (only in Another.md)
        let results_before = index.search("test-vault", "python", 10).unwrap();
        assert_eq!(results_before.len(), 1);

        // Remove the file from index
        index.remove_file("test-vault", "Another.md").unwrap();

        // Now should not find Python
        let results_after = index.search("test-vault", "python", 10).unwrap();
        assert!(results_after.is_empty());
    }

    #[test]
    fn test_remove_vault() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        // Search works before removal
        let results_before = index.search("test-vault", "rust", 10);
        assert!(results_before.is_ok());

        // Remove the vault
        index.remove_vault("test-vault").unwrap();

        // Search should fail after removal (vault not found)
        let results_after = index.search("test-vault", "rust", 10);
        assert!(results_after.is_err());
    }

    #[test]
    fn test_search_nonexistent_vault() {
        let index = SearchIndex::new();

        let result = index.search("nonexistent-vault", "test", 10);

        assert!(result.is_err());
    }

    #[test]
    fn test_nested_file_search() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "nested", 10).unwrap();

        // Should find the nested file
        assert!(!results.is_empty());

        let nested_result = results.iter().find(|r| r.path.contains("folder/"));
        assert!(
            nested_result.is_some(),
            "Expected to find nested file in results"
        );
    }

    #[test]
    fn test_line_numbers_correct() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "multiple", 10).unwrap();

        // "multiple" appears in Note.md on line 4 ("It contains multiple lines.")
        assert!(!results.is_empty());

        let note_result = results.iter().find(|r| r.path == "Note.md");
        assert!(note_result.is_some());

        let match_info = &note_result.unwrap().matches[0];
        assert_eq!(
            match_info.line_number, 4,
            "Expected line 4, got {}",
            match_info.line_number
        );
    }

    #[test]
    fn test_max_matches_per_file() {
        let temp_dir = TempDir::new().unwrap();
        let vault = temp_dir.path();

        // Create a file with many occurrences of the same word
        let mut content = String::from("# Many Matches\n\n");
        for i in 0..20 {
            content.push_str(&format!("Line {} has the word test in it.\n", i));
        }
        fs::write(vault.join("ManyMatches.md"), content).unwrap();

        let index = SearchIndex::new();
        index
            .index_vault("test-vault", vault.to_str().unwrap())
            .unwrap();

        let results = index.search("test-vault", "test", 10).unwrap();

        // Should have at most 10 matches per file
        for result in &results {
            assert!(
                result.matches.len() <= 10,
                "Expected max 10 matches, got {}",
                result.matches.len()
            );
        }
    }

    #[test]
    fn test_special_characters_in_search() {
        let temp_dir = TempDir::new().unwrap();
        let vault = temp_dir.path();

        fs::write(
            vault.join("Special.md"),
            "# Special Characters\n\nC++ is a language. C# is too. $money$ matters.",
        )
        .unwrap();

        let index = SearchIndex::new();
        index
            .index_vault("test-vault", vault.to_str().unwrap())
            .unwrap();

        // Search for strings with special characters
        let results_cpp = index.search("test-vault", "c++", 10).unwrap();
        assert!(!results_cpp.is_empty(), "Should find C++");

        let results_money = index.search("test-vault", "$money$", 10).unwrap();
        assert!(!results_money.is_empty(), "Should find $money$");
    }

    #[test]
    fn test_unicode_search() {
        let temp_dir = TempDir::new().unwrap();
        let vault = temp_dir.path();

        fs::write(
            vault.join("Unicode.md"),
            "# Unicode Test\n\nこんにちは means hello.\nEmoji: 🦀 is a crab.",
        )
        .unwrap();

        let index = SearchIndex::new();
        index
            .index_vault("test-vault", vault.to_str().unwrap())
            .unwrap();

        let results_jp = index.search("test-vault", "こんにちは", 10).unwrap();
        assert!(!results_jp.is_empty(), "Should find Japanese text");

        let results_emoji = index.search("test-vault", "🦀", 10).unwrap();
        assert!(!results_emoji.is_empty(), "Should find emoji");
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap().to_string();
        let index = SearchIndex::new();
        index.index_vault("test-vault", &vault_path).unwrap();

        let index_clone1 = index.clone();
        let index_clone2 = index.clone();

        // Spawn threads that read concurrently
        let handle1 = thread::spawn(move || {
            for _ in 0..100 {
                let _ = index_clone1.search("test-vault", "rust", 10);
            }
        });

        let handle2 = thread::spawn(move || {
            for _ in 0..100 {
                let _ = index_clone2.search("test-vault", "note", 10);
            }
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // If we get here without panicking, concurrent access works
    }
}
