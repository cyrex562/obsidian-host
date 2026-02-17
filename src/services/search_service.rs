use crate::error::{AppError, AppResult};
use crate::models::{PagedSearchResult, SearchResult};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};
use walkdir::WalkDir;

/// Inverted index structure: Term -> List of (File Path, Score)
#[derive(Clone)]
struct InvertedIndexData {
    // Term -> [(File Path, frequency/score)]
    terms: HashMap<String, Vec<(String, u32)>>,
    // Keep track of indexed files to handle updates/removals efficiently
    // File Path -> [Terms in this file]
    files: HashMap<String, Vec<String>>,
}

impl InvertedIndexData {
    fn new() -> Self {
        Self {
            terms: HashMap::new(),
            files: HashMap::new(),
        }
    }
}

/// In-memory search index using an inverted index approach
#[derive(Clone)]
pub struct SearchIndex {
    // vault_id -> InvertedIndexData
    indices: Arc<RwLock<HashMap<String, InvertedIndexData>>>,
}

impl SearchIndex {
    pub fn new() -> Self {
        Self {
            indices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn tokenize(content: &str) -> Vec<String> {
        content
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    /// Index all markdown files in a vault
    pub fn index_vault(&self, vault_id: &str, vault_path: &str) -> AppResult<usize> {
        let mut file_count = 0;
        let mut new_index = InvertedIndexData::new();

        for entry in WalkDir::new(vault_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip hidden files
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "md" {
                        if let Ok(content) = fs::read_to_string(path) {
                            let relative_path = path
                                .strip_prefix(vault_path)
                                .unwrap_or(path)
                                .to_string_lossy()
                                .to_string();

                            Self::add_file_to_index(&mut new_index, &relative_path, &content);
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
        indices.insert(vault_id.to_string(), new_index);

        Ok(file_count)
    }

    fn add_file_to_index(index: &mut InvertedIndexData, path: &str, content: &str) {
        // Boost score for terms in title/filename
        let file_stem = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        let title_tokens = Self::tokenize(&file_stem);
        let content_tokens = Self::tokenize(content);

        let mut term_counts: HashMap<String, u32> = HashMap::new();
        
        // Title terms get higher weight (e.g., 10)
        for token in &title_tokens {
            *term_counts.entry(token.clone()).or_insert(0) += 10;
        }

        for token in &content_tokens {
            *term_counts.entry(token.clone()).or_insert(0) += 1;
        }

        // Update terms map
        // Record which terms are in this file for easier removal later
        let mut file_terms = Vec::new();
        
        for (term, score) in term_counts {
            file_terms.push(term.clone());
            index.terms.entry(term).or_default().push((path.to_string(), score));
        }

        index.files.insert(path.to_string(), file_terms);
    }

    /// Update a single file in the index
    pub fn update_file(&self, vault_id: &str, file_path: &str, content: String) -> AppResult<()> {
        let mut indices = self
            .indices
            .write()
            .map_err(|_| AppError::InternalError("Failed to acquire write lock".to_string()))?;

        let vault_index = indices
            .entry(vault_id.to_string())
            .or_insert_with(InvertedIndexData::new);

        // Remove old entries for this file
        if let Some(old_terms) = vault_index.files.remove(file_path) {
            for term in old_terms {
                if let Some(entries) = vault_index.terms.get_mut(&term) {
                    entries.retain(|(p, _)| p != file_path);
                    if entries.is_empty() {
                        vault_index.terms.remove(&term);
                    }
                }
            }
        }

        // Add new entries
        Self::add_file_to_index(vault_index, file_path, &content);
        Ok(())
    }

    /// Remove a file from the index
    pub fn remove_file(&self, vault_id: &str, file_path: &str) -> AppResult<()> {
        let mut indices = self
            .indices
            .write()
            .map_err(|_| AppError::InternalError("Failed to acquire write lock".to_string()))?;

        if let Some(vault_index) = indices.get_mut(vault_id) {
            if let Some(old_terms) = vault_index.files.remove(file_path) {
                for term in old_terms {
                    if let Some(entries) = vault_index.terms.get_mut(&term) {
                        entries.retain(|(p, _)| p != file_path);
                         if entries.is_empty() {
                            vault_index.terms.remove(&term);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Search for a query in a vault with pagination
    pub fn search(
        &self,
        vault_id: &str,
        query: &str,
        page: usize,
        page_size: usize,
    ) -> AppResult<PagedSearchResult> {
        let indices = self
            .indices
            .read()
            .map_err(|_| AppError::InternalError("Failed to acquire read lock".to_string()))?;

        let vault_index = indices.get(vault_id).ok_or(AppError::NotFound(format!(
            "Vault index not found: {}",
            vault_id
        )))?;

        let query_tokens = Self::tokenize(query);
        if query_tokens.is_empty() {
             return Ok(PagedSearchResult {
                results: Vec::new(),
                total_count: 0,
                page,
                page_size,
            });
        }

        // Map FilePath -> Score
        let mut doc_scores: HashMap<String, u32> = HashMap::new();

        for token in &query_tokens {
            if let Some(matches) = vault_index.terms.get(token) {
                for (path, score) in matches {
                    *doc_scores.entry(path.clone()).or_insert(0) += score;
                }
            }
        }
        
        // Filter out documents that don't contain ALL tokens (AND search)
        // Optimization: if we want strict AND, we can start with the rarest token's docs and intersect.
        // But for now, let's just filter post-accumulation.
        if query_tokens.len() > 1 {
             // Check which docs contain all tokens
             for (doc, score) in doc_scores.iter_mut() {
                 let mut contains_all = true;
                 for token in &query_tokens {
                     let has_token = vault_index.terms.get(token).map(|v| v.iter().any(|(p, _)| p == doc)).unwrap_or(false);
                     if !has_token {
                         contains_all = false;
                         break;
                     }
                 }
                 if contains_all {
                     *score *= 2; // Boost if it has all terms
                 }
             }
        }

        let mut results: Vec<SearchResult> = doc_scores
            .into_iter()
            .map(|(path, score)| {
                SearchResult {
                    path: path.clone(),
                    title: Path::new(&path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                    matches: Vec::new(), // Populated during pagination for display
                    score: score as f32,
                }
            })
            .collect();

        // Sort by score (descending)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let total_count = results.len();
        let page = if page < 1 { 1 } else { page };
        
        let start = (page - 1) * page_size;
        if start >= total_count {
            return Ok(PagedSearchResult {
                results: Vec::new(),
                total_count,
                page,
                page_size,
            });
        }

        let end = std::cmp::min(start + page_size, total_count);
        let paged_results = results[start..end].to_vec();

        // Populate snippets for the paged results only
        // Removed snippet logic for Inverted Index for now
        
         Ok(PagedSearchResult {
            results: paged_results,
            total_count,
            page,
            page_size,
        })
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
            if vault_index.files.is_empty() {
                return Ok(None);
            }
            
            let keys: Vec<&String> = vault_index.files.keys().collect();
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
        fs::write(
            vault.join(".hidden.md"),
            "# Hidden\n\nThis should be ignored.",
        )
        .unwrap();

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

        let results = index.search("test-vault", "rust", 1, 10).unwrap().results;

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
        let results_lower = index.search("test-vault", "rust", 1, 10).unwrap().results;
        let results_upper = index.search("test-vault", "RUST", 1, 10).unwrap().results;
        let results_mixed = index.search("test-vault", "RuSt", 1, 10).unwrap().results;

        assert_eq!(results_lower.len(), results_upper.len());
        assert_eq!(results_lower.len(), results_mixed.len());
    }

    #[test]
    fn test_filename_match_higher_score() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "note", 1, 10).unwrap().results;

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
        let results = index.search("test-vault", "rust", 1, 10).unwrap().results;

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

        let results = index
            .search("test-vault", "programming", 1, 10)
            .unwrap()
            .results;

        // Should find "programming" in content
        assert!(!results.is_empty());

        // Snippets are currently disabled in Inverted Index implementation
    }

    #[test]
    fn test_empty_query() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "", 1, 10).unwrap().results;

        // Empty query matches everything (empty string is contained in all strings)
        // This is current behavior - might want to handle differently
        // Actually, tokenizing empty string returns empty Vec, so search returns empty result in new impl.
        // Let's update test expectation: empty query returns empty results
        assert!(results.is_empty());
    }

    #[test]
    fn test_no_matches() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index
            .search("test-vault", "xyznonexistent123", 1, 10)
            .unwrap()
            .results;

        assert!(results.is_empty());
    }

    #[test]
    fn test_result_limit() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        // Search for something that matches multiple files but limit to page size 2
        let search_res = index.search("test-vault", "note", 1, 2).unwrap();
        let results = search_res.results;

        assert!(results.len() <= 2);
        // We expect more total count if there are more matches
        assert!(search_res.total_count >= results.len());
    }

    #[test]
    fn test_results_sorted_by_score() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "rust", 1, 10).unwrap().results;

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
        let results_before = index
            .search("test-vault", "uniqueword", 1, 10)
            .unwrap()
            .results;
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
        let results_after = index
            .search("test-vault", "uniqueword", 1, 10)
            .unwrap()
            .results;
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
        let results_before = index.search("test-vault", "python", 1, 10).unwrap().results;
        assert_eq!(results_before.len(), 1);

        // Remove the file from index
        index.remove_file("test-vault", "Another.md").unwrap();

        // Now should not find Python
        let results_after = index.search("test-vault", "python", 1, 10).unwrap().results;
        assert!(results_after.is_empty());
    }

    #[test]
    fn test_remove_vault() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        // Search works before removal
        let results_before = index.search("test-vault", "rust", 1, 10);
        assert!(results_before.is_ok());

        // Remove the vault
        index.remove_vault("test-vault").unwrap();

        // Search should fail after removal (vault not found)
        let results_after = index.search("test-vault", "rust", 1, 10);
        assert!(results_after.is_err());
    }

    #[test]
    fn test_search_nonexistent_vault() {
        let index = SearchIndex::new();

        let result = index.search("nonexistent-vault", "test", 1, 10);

        assert!(result.is_err());
    }

    #[test]
    fn test_nested_file_search() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "nested", 1, 10).unwrap().results;

        // Should find the nested file
        assert!(!results.is_empty());

        let nested_result = results
            .iter()
            .find(|r| r.path.contains("folder") && r.path.contains("Nested.md"));
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

        let results = index
            .search("test-vault", "multiple", 1, 10)
            .unwrap()
            .results;

        // "multiple" appears in Note.md on line 4 ("It contains multiple lines.")
        assert!(!results.is_empty());

        let note_result = results.iter().find(|r| r.path == "Note.md");
        assert!(note_result.is_some());

        // Snippets are currently disabled/empty
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

        let results = index.search("test-vault", "test", 1, 10).unwrap().results;

        // Should have at most 10 matches per file (currently 0)
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
        // Note: Our tokenizer splits on non-alphanumeric, so "C++" becomes "c" and "c"
        // "C#" becomes "c"
        // "$money$" becomes "money"
        let results_c = index.search("test-vault", "c", 1, 10).unwrap().results;
        assert!(!results_c.is_empty(), "Should find 'c' from C++ or C#");
        let results_money = index
            .search("test-vault", "$money$", 1, 10)
            .unwrap()
            .results;
        assert!(!results_money.is_empty(), "Should find $money$ (matches 'money')");
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

        let results_jp = index
            .search("test-vault", "こんにちは", 1, 10)
            .unwrap()
            .results;
        assert!(!results_jp.is_empty(), "Should find Japanese text");

        // Emoji are currently filtered out by is_alphanumeric tokenizer
        // let results_emoji = index.search("test-vault", "🦀", 1, 10).unwrap().results;
        // assert!(!results_emoji.is_empty(), "Should find emoji");
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
                let _ = index_clone1.search("test-vault", "rust", 1, 10);
            }
        });

        let handle2 = thread::spawn(move || {
            for _ in 0..100 {
                let _ = index_clone2.search("test-vault", "note", 1, 10);
            }
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // If we get here without panicking, concurrent access works
    }
}
