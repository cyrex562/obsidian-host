use crate::error::{AppError, AppResult};
use crate::models::{PagedSearchResult, SearchMatch, SearchResult};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, QueryParser};
use tantivy::schema::Value as TantivyValue;
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, STORED, STRING,
};
use tantivy::{doc, Index, IndexReader, ReloadPolicy, TantivyDocument, Term};
use walkdir::WalkDir;

// ── Entity metadata ──────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct EntityMeta {
    entity_type: Option<String>,
    labels: Vec<String>,
    extra_text: String,
}

fn extract_entity_meta(content: &str) -> EntityMeta {
    if !content.starts_with("---") {
        return EntityMeta::default();
    }
    let end = match content.find("\n---") {
        Some(e) => e,
        None => return EntityMeta::default(),
    };
    if end < 4 {
        return EntityMeta::default();
    }
    let yaml_block = &content[4..end];

    let entity_type = yaml_block.lines().find_map(|line| {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("codex_type:") {
            let val = rest.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                Some(val.to_string())
            } else {
                None
            }
        } else {
            None
        }
    });

    if entity_type.is_none() {
        return EntityMeta::default();
    }

    let mut labels = Vec::new();
    let mut in_labels = false;
    for line in yaml_block.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("codex_labels:") {
            in_labels = true;
            continue;
        }
        if in_labels {
            if let Some(item) = trimmed.strip_prefix("- ") {
                labels.push(item.trim_matches('"').trim_matches('\'').to_string());
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                in_labels = false;
            }
        }
    }

    let reserved = ["codex_type", "codex_labels", "codex_plugin"];
    let mut extra_parts: Vec<String> = Vec::new();
    for line in yaml_block.lines() {
        let trimmed = line.trim();
        if let Some(colon_pos) = trimmed.find(':') {
            let key = trimmed[..colon_pos].trim();
            let val = trimmed[colon_pos + 1..].trim();
            if !reserved.contains(&key)
                && !val.is_empty()
                && !val.starts_with('[')
                && !val.starts_with('{')
            {
                let clean = val.trim_matches('"').trim_matches('\'');
                if !clean.is_empty() {
                    extra_parts.push(clean.to_string());
                }
            }
        }
    }

    EntityMeta {
        entity_type,
        labels,
        extra_text: extra_parts.join(" "),
    }
}

// ── Tantivy schema ───────────────────────────────────────────────────────────

struct IndexFields {
    path: Field,
    title: Field,
    body: Field,
    entity_type: Field,
    labels: Field,
}

fn build_schema() -> (Schema, IndexFields) {
    let mut sb = Schema::builder();

    let text_opts = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("default")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();

    let path_field = sb.add_text_field("path", STRING | STORED);
    let title_field = sb.add_text_field("title", text_opts.clone());
    let body_field = sb.add_text_field("body", text_opts);
    let entity_type_field = sb.add_text_field("entity_type", STORED);
    let labels_field = sb.add_text_field("labels", STORED);

    let schema = sb.build();
    let fields = IndexFields {
        path: path_field,
        title: title_field,
        body: body_field,
        entity_type: entity_type_field,
        labels: labels_field,
    };
    (schema, fields)
}

// ── VaultIndex ───────────────────────────────────────────────────────────────

struct VaultIndex {
    index: Index,
    reader: IndexReader,
    vault_path: String,
    fields: IndexFields,
}

// ── SearchIndex ──────────────────────────────────────────────────────────────

/// Tantivy-backed full-text search index, one per vault.
#[derive(Clone)]
pub struct SearchIndex {
    vaults: Arc<RwLock<HashMap<String, VaultIndex>>>,
    /// `None` → in-RAM index (test mode). `Some(path)` → disk MmapDirectory.
    base_dir: Option<PathBuf>,
}

impl SearchIndex {
    pub fn new() -> Self {
        #[cfg(test)]
        let base_dir: Option<PathBuf> = None;

        #[cfg(not(test))]
        let base_dir: Option<PathBuf> = {
            let dir = PathBuf::from("./data/indices");
            std::fs::create_dir_all(&dir).ok();
            Some(dir)
        };

        Self {
            vaults: Arc::new(RwLock::new(HashMap::new())),
            base_dir,
        }
    }

    fn open_index(&self, vault_id: &str, schema: Schema) -> AppResult<Index> {
        match &self.base_dir {
            Some(base) => {
                let dir = base.join(vault_id);
                std::fs::create_dir_all(&dir).map_err(|e| {
                    AppError::InternalError(format!("Failed to create index dir: {e}"))
                })?;
                let idx = match Index::open_in_dir(&dir) {
                    Ok(existing) => existing,
                    Err(_) => Index::create_in_dir(&dir, schema).map_err(|e| {
                        AppError::InternalError(format!("Failed to create index: {e}"))
                    })?,
                };
                Ok(idx)
            }
            None => Ok(Index::create_in_ram(schema)),
        }
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Index all markdown files in a vault. Returns the count of indexed files.
    pub fn index_vault(&self, vault_id: &str, vault_path: &str) -> AppResult<usize> {
        let (schema, fields) = build_schema();
        let index = self.open_index(vault_id, schema)?;

        let mut writer = index
            .writer::<TantivyDocument>(50_000_000)
            .map_err(|e| AppError::InternalError(format!("Writer error: {e}")))?;

        writer
            .delete_all_documents()
            .map_err(|e| AppError::InternalError(format!("Delete all error: {e}")))?;

        let mut count = 0usize;
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
                    let title = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    let meta = extract_entity_meta(&content);

                    writer
                        .add_document(doc!(
                            fields.path => rel,
                            fields.title => title,
                            fields.body => content,
                            fields.entity_type => meta.entity_type.unwrap_or_default(),
                            fields.labels => meta.labels.join(" "),
                        ))
                        .map_err(|e| AppError::InternalError(format!("Add doc error: {e}")))?;
                    count += 1;
                }
            }
        }

        writer
            .commit()
            .map_err(|e| AppError::InternalError(format!("Commit error: {e}")))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| AppError::InternalError(format!("Reader error: {e}")))?;

        let mut vaults = self
            .vaults
            .write()
            .map_err(|_| AppError::InternalError("Lock error".to_string()))?;
        vaults.insert(
            vault_id.to_string(),
            VaultIndex {
                index,
                reader,
                vault_path: vault_path.to_string(),
                fields,
            },
        );

        Ok(count)
    }

    /// Update (or insert) a single file in the index.
    pub fn update_file(&self, vault_id: &str, file_path: &str, content: String) -> AppResult<()> {
        let vaults = self
            .vaults
            .read()
            .map_err(|_| AppError::InternalError("Lock error".to_string()))?;
        let vi = match vaults.get(vault_id) {
            Some(v) => v,
            None => return Ok(()),
        };

        let mut writer = vi
            .index
            .writer::<TantivyDocument>(50_000_000)
            .map_err(|e| AppError::InternalError(format!("Writer error: {e}")))?;

        writer.delete_term(Term::from_field_text(vi.fields.path, file_path));

        let title = Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let meta = extract_entity_meta(&content);

        writer
            .add_document(doc!(
                vi.fields.path => file_path.to_string(),
                vi.fields.title => title,
                vi.fields.body => content,
                vi.fields.entity_type => meta.entity_type.unwrap_or_default(),
                vi.fields.labels => meta.labels.join(" "),
            ))
            .map_err(|e| AppError::InternalError(format!("Add doc error: {e}")))?;

        writer
            .commit()
            .map_err(|e| AppError::InternalError(format!("Commit error: {e}")))?;

        vi.reader
            .reload()
            .map_err(|e| AppError::InternalError(format!("Reload error: {e}")))?;

        Ok(())
    }

    /// Remove a single file from the index.
    pub fn remove_file(&self, vault_id: &str, file_path: &str) -> AppResult<()> {
        let vaults = self
            .vaults
            .read()
            .map_err(|_| AppError::InternalError("Lock error".to_string()))?;
        let vi = match vaults.get(vault_id) {
            Some(v) => v,
            None => return Ok(()),
        };

        let mut writer = vi
            .index
            .writer::<TantivyDocument>(50_000_000)
            .map_err(|e| AppError::InternalError(format!("Writer error: {e}")))?;

        writer.delete_term(Term::from_field_text(vi.fields.path, file_path));

        writer
            .commit()
            .map_err(|e| AppError::InternalError(format!("Commit error: {e}")))?;

        vi.reader
            .reload()
            .map_err(|e| AppError::InternalError(format!("Reload error: {e}")))?;

        Ok(())
    }

    /// Remove an entire vault from the in-memory map.
    pub fn remove_vault(&self, vault_id: &str) -> AppResult<()> {
        let mut vaults = self
            .vaults
            .write()
            .map_err(|_| AppError::InternalError("Lock error".to_string()))?;
        vaults.remove(vault_id);
        Ok(())
    }

    /// Full-text search with pagination.
    pub fn search(
        &self,
        vault_id: &str,
        query: &str,
        page: usize,
        page_size: usize,
    ) -> AppResult<PagedSearchResult> {
        use tantivy::DocAddress;

        let query_lower = query.to_lowercase();

        // Phase 1: Acquire lock, clone everything needed, release lock immediately.
        let (searcher, vault_path, fp_field, title_field, body_field, et_field, lbl_field, index) = {
            let vaults = self
                .vaults
                .read()
                .map_err(|_| AppError::InternalError("Lock error".to_string()))?;
            let vi = vaults
                .get(vault_id)
                .ok_or_else(|| AppError::NotFound(format!("Vault index not found: {vault_id}")))?;
            let searcher = vi.reader.searcher();
            (
                searcher,
                vi.vault_path.clone(),
                vi.fields.path,
                vi.fields.title,
                vi.fields.body,
                vi.fields.entity_type,
                vi.fields.labels,
                vi.index.clone(),
            )
        };

        // Phase 2: Tantivy query — collect (path, doc_address) pairs.
        let candidates: Vec<(String, DocAddress)> = if query.is_empty() {
            searcher
                .search(&AllQuery, &TopDocs::with_limit(10_000))
                .map_err(|e| AppError::InternalError(format!("Search error: {e}")))?
                .into_iter()
                .filter_map(|(_, addr)| {
                    let doc: TantivyDocument = searcher.doc(addr).ok()?;
                    let path = TantivyValue::as_str(&doc.get_first(fp_field)?)?.to_string();
                    Some((path, addr))
                })
                .collect()
        } else {
            let mut qp = QueryParser::for_index(&index, vec![title_field, body_field]);
            qp.set_field_boost(title_field, 3.0);

            match qp.parse_query(&query_lower) {
                Ok(tq) => searcher
                    .search(&tq, &TopDocs::with_limit(1_000))
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|(_, addr)| {
                        let doc: TantivyDocument = searcher.doc(addr).ok()?;
                        let path = TantivyValue::as_str(&doc.get_first(fp_field)?)?.to_string();
                        Some((path, addr))
                    })
                    .collect(),
                Err(_) => Vec::new(),
            }
        };

        // Phase 3: Build results using stored body content from tantivy
        // (avoids stale disk reads when update_file is called without updating disk).
        let mut results: Vec<SearchResult> = candidates
            .iter()
            .filter_map(|(file_path, doc_addr)| {
                let doc: TantivyDocument = searcher.doc(*doc_addr).ok()?;
                let content = TantivyValue::as_str(&doc.get_first(body_field)?)
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                let file_name = Path::new(file_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                let mut matches: Vec<SearchMatch> = Vec::new();
                let mut score = 0.0f32;

                if file_name.to_lowercase().contains(&query_lower) {
                    score += 10.0;
                }

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
                        if matches.len() >= 10 {
                            break;
                        }
                    }
                }

                if matches.is_empty() && score == 0.0 {
                    if query.is_empty() {
                        score = 1.0;
                    } else {
                        return None;
                    }
                }

                // Entity metadata from stored fields.
                let entity_type = doc
                    .get_first(et_field)
                    .and_then(|v| TantivyValue::as_str(&v))
                    .and_then(|s| {
                        if s.is_empty() {
                            None
                        } else {
                            Some(s.to_string())
                        }
                    });
                let labels_str = doc
                    .get_first(lbl_field)
                    .and_then(|v| TantivyValue::as_str(&v))
                    .unwrap_or("");
                let labels: Vec<String> = labels_str
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                Some(SearchResult {
                    path: file_path.clone(),
                    title: file_name.to_string(),
                    matches,
                    score,
                    entity_type,
                    labels,
                })
            })
            .collect();

        // Phase 4: Fallback disk scan — only for queries that tantivy's tokenizer
        // strips entirely (e.g. emoji). Tokenizable queries trust tantivy's result.
        if results.is_empty() && !query.is_empty() && !has_tokenizable_chars(&query_lower) {
            results = Self::fallback_disk_search(&vault_path, query)?;
        }

        // ── Sort descending by score ──────────────────────────────────────────
        results.par_sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total_count = results.len();
        let page = page.max(1);
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
        Ok(PagedSearchResult {
            results: results[start..end].to_vec(),
            total_count,
            page,
            page_size,
        })
    }

    /// Return a random markdown file path from the vault.
    pub fn get_random_file(&self, vault_id: &str) -> AppResult<Option<String>> {
        let (searcher, fp_field) = {
            let vaults = self
                .vaults
                .read()
                .map_err(|_| AppError::InternalError("Lock error".to_string()))?;
            let vi = match vaults.get(vault_id) {
                Some(v) => v,
                None => return Ok(None),
            };
            (vi.reader.searcher(), vi.fields.path)
        };

        let top_docs = searcher
            .search(&AllQuery, &TopDocs::with_limit(10_000))
            .map_err(|e| AppError::InternalError(format!("Search error: {e}")))?;

        if top_docs.is_empty() {
            return Ok(None);
        }

        use rand::seq::IndexedRandom;
        let mut rng = rand::rng();
        if let Some((_, addr)) = top_docs.choose(&mut rng) {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(*addr) {
                let path = doc
                    .get_first(fp_field)
                    .and_then(|v| TantivyValue::as_str(&v))
                    .map(|s| s.to_string());
                return Ok(path);
            }
        }
        Ok(None)
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    /// Linear substring scan used as fallback when tantivy finds nothing
    /// (e.g. emoji, certain special characters stripped by tokenizer).
    fn fallback_disk_search(vault_path: &str, query: &str) -> AppResult<Vec<SearchResult>> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

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
                    let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

                    let mut matches = Vec::new();
                    let mut score = 0.0f32;

                    if file_name.to_lowercase().contains(&query_lower) {
                        score += 10.0;
                    }

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
                            if matches.len() >= 10 {
                                break;
                            }
                        }
                    }

                    if !matches.is_empty() || score > 0.0 {
                        let meta = extract_entity_meta(&content);
                        results.push(SearchResult {
                            path: rel,
                            title: file_name.to_string(),
                            matches,
                            score,
                            entity_type: meta.entity_type,
                            labels: meta.labels,
                        });
                    }
                }
            }
        }

        Ok(results)
    }
}

fn has_tokenizable_chars(s: &str) -> bool {
    s.chars().any(|c| c.is_alphanumeric())
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_vault() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let vault = temp_dir.path();

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

        fs::create_dir(vault.join("folder")).unwrap();
        fs::write(
            vault.join("folder/Nested.md"),
            "# Nested Note\n\nThis is a nested file about rust programming.",
        )
        .unwrap();

        fs::write(vault.join("readme.txt"), "This is a text file.").unwrap();
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
        assert_eq!(count, 4);
    }

    #[test]
    fn test_basic_search() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "rust", 1, 10).unwrap().results;
        assert!(!results.is_empty());
        assert!(results.len() >= 3);
    }

    #[test]
    fn test_case_insensitive_search() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

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
        assert!(!results.is_empty());

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

        let results = index.search("test-vault", "rust", 1, 10).unwrap().results;
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

        let results = index.search("test-vault", "", 1, 10).unwrap().results;
        assert!(!results.is_empty());
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

        let search_res = index.search("test-vault", "note", 1, 2).unwrap();
        let results = search_res.results;

        assert!(results.len() <= 2);
        assert!(search_res.total_count >= results.len());
    }

    #[test]
    fn test_results_sorted_by_score() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results = index.search("test-vault", "rust", 1, 10).unwrap().results;

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

        let results_before = index
            .search("test-vault", "uniqueword", 1, 10)
            .unwrap()
            .results;
        assert!(results_before.is_empty());

        index
            .update_file(
                "test-vault",
                "Note.md",
                "# Updated Note\n\nThis contains uniqueword now.".to_string(),
            )
            .unwrap();

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

        let results_before = index.search("test-vault", "python", 1, 10).unwrap().results;
        assert_eq!(results_before.len(), 1);

        index.remove_file("test-vault", "Another.md").unwrap();

        let results_after = index.search("test-vault", "python", 1, 10).unwrap().results;
        assert!(results_after.is_empty());
    }

    #[test]
    fn test_remove_vault() {
        let temp = create_test_vault();
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        index.index_vault("test-vault", vault_path).unwrap();

        let results_before = index.search("test-vault", "rust", 1, 10);
        assert!(results_before.is_ok());

        index.remove_vault("test-vault").unwrap();

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

        let results_cpp = index.search("test-vault", "c++", 1, 10).unwrap().results;
        assert!(!results_cpp.is_empty(), "Should find C++");

        let results_money = index
            .search("test-vault", "$money$", 1, 10)
            .unwrap()
            .results;
        assert!(!results_money.is_empty(), "Should find $money$");
    }

    #[test]
    fn test_unicode_search() {
        let temp_dir = TempDir::new().unwrap();
        let vault = temp_dir.path();

        fs::write(
            vault.join("Unicode.md"),
            "# Unicode Test\n\n\u{3053}\u{3093}\u{306B}\u{3061}\u{306F} means hello.\nEmoji: \u{1F980} is a crab.",
        )
        .unwrap();

        let index = SearchIndex::new();
        index
            .index_vault("test-vault", vault.to_str().unwrap())
            .unwrap();

        let results_jp = index
            .search(
                "test-vault",
                "\u{3053}\u{3093}\u{306B}\u{3061}\u{306F}",
                1,
                10,
            )
            .unwrap()
            .results;
        assert!(!results_jp.is_empty(), "Should find Japanese text");

        let results_emoji = index
            .search("test-vault", "\u{1F980}", 1, 10)
            .unwrap()
            .results;
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
    }

    // ── extract_entity_meta tests ─────────────────────────────────────────────

    #[test]
    fn test_extract_entity_meta_no_frontmatter() {
        let content = "# Just a plain markdown file\n\nNo frontmatter here.";
        let meta = extract_entity_meta(content);
        assert!(meta.entity_type.is_none());
        assert!(meta.labels.is_empty());
        assert!(meta.extra_text.is_empty());
    }

    #[test]
    fn test_extract_entity_meta_no_codex_type() {
        let content = "---\ntitle: My Note\nauthor: Alice\n---\n# Content";
        let meta = extract_entity_meta(content);
        assert!(meta.entity_type.is_none());
        assert!(meta.labels.is_empty());
    }

    #[test]
    fn test_extract_entity_meta_with_codex_type() {
        let content = "---\ncodex_type: character\nfull_name: Alice\n---\n# Content";
        let meta = extract_entity_meta(content);
        assert_eq!(meta.entity_type.as_deref(), Some("character"));
    }

    #[test]
    fn test_extract_entity_meta_with_labels() {
        let content =
            "---\ncodex_type: character\ncodex_labels:\n- graphable\n- person\n---\n# Content";
        let meta = extract_entity_meta(content);
        assert_eq!(meta.entity_type.as_deref(), Some("character"));
        assert!(meta.labels.contains(&"graphable".to_string()));
        assert!(meta.labels.contains(&"person".to_string()));
        assert_eq!(meta.labels.len(), 2);
    }

    #[test]
    fn test_extract_entity_meta_extra_text_from_string_fields() {
        let content =
            "---\ncodex_type: character\nfull_name: Alice Smith\nstatus: Active\n---\n# Content";
        let meta = extract_entity_meta(content);
        assert!(meta.extra_text.contains("Alice Smith"));
        assert!(meta.extra_text.contains("Active"));
    }

    #[test]
    fn test_extract_entity_meta_reserved_keys_not_in_extra_text() {
        let content =
            "---\ncodex_type: character\ncodex_plugin: worldbuilding\nfull_name: Alice\n---\n# Content";
        let meta = extract_entity_meta(content);
        assert!(!meta.extra_text.contains("worldbuilding"));
        assert!(!meta.extra_text.contains("character"));
    }

    #[test]
    fn test_extract_entity_meta_empty_frontmatter() {
        let content = "---\n---\n# Content";
        let meta = extract_entity_meta(content);
        assert!(meta.entity_type.is_none());
    }

    #[test]
    fn test_search_result_has_entity_type_after_index() {
        let temp = TempDir::new().unwrap();
        let vault = temp.path();
        let content = "---\ncodex_type: character\ncodex_labels:\n- graphable\nfull_name: Alice Smith\n---\n# Alice Smith\n\nA brave adventurer.";
        fs::write(vault.join("alice.md"), content).unwrap();

        let index = SearchIndex::new();
        index
            .index_vault("vault1", vault.to_str().unwrap())
            .unwrap();

        let results = index.search("vault1", "alice", 1, 10).unwrap().results;
        assert!(!results.is_empty());
        let result = &results[0];
        assert_eq!(result.entity_type.as_deref(), Some("character"));
        assert!(result.labels.contains(&"graphable".to_string()));
    }

    #[test]
    fn test_search_result_no_entity_type_for_plain_file() {
        let temp = TempDir::new().unwrap();
        let vault = temp.path();
        let content = "# Just a note\n\nNo entity metadata here.";
        fs::write(vault.join("note.md"), content).unwrap();

        let index = SearchIndex::new();
        index
            .index_vault("vault1", vault.to_str().unwrap())
            .unwrap();

        let results = index.search("vault1", "note", 1, 10).unwrap().results;
        assert!(!results.is_empty());
        let result = &results[0];
        assert!(result.entity_type.is_none());
        assert!(result.labels.is_empty());
    }
}
