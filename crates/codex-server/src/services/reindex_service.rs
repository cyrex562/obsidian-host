use crate::db::Database;
use crate::error::AppResult;
use crate::services::entity_service::EntityService;
use crate::services::relation_service::RelationService;
use chrono::Utc;
use std::path::Path;
use tokio::fs;
use tracing::{debug, error, info, warn};

pub struct ReindexService;

impl ReindexService {
    /// Full two-pass reindex for a vault.
    ///
    /// Pass 1: Walk vault directory, parse frontmatter, upsert entities.
    ///         Remove stale entities (path on disk deleted since last index).
    /// Pass 2: For every entity just indexed, sync relations from fields.
    ///         Unresolved refs are silently dropped (fixed on a subsequent run).
    pub async fn reindex_vault(db: &Database, vault_id: &str, vault_path: &str) -> AppResult<i64> {
        let start = Utc::now();
        info!("Starting reindex of vault {vault_id} at {vault_path}");

        // --- Pass 1: index all entities ---
        let md_files = collect_md_files(vault_path).await;

        let mut indexed_count = 0usize;
        let mut error_count = 0usize;
        let mut visited_paths: Vec<String> = Vec::new();

        for abs_path in &md_files {
            let rel_path = match abs_path.strip_prefix(vault_path) {
                Some(p) => p.trim_start_matches('/').to_string(),
                None => {
                    warn!("Could not make {abs_path} relative to {vault_path}");
                    continue;
                }
            };

            let content = match fs::read_to_string(abs_path).await {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to read {abs_path}: {e}");
                    error_count += 1;
                    continue;
                }
            };

            // Parse frontmatter
            if let Some(fm) = EntityService::parse_frontmatter(&content) {
                // Only upsert if it has a codex_type
                if fm.get("codex_type").is_some() {
                    // Use file modification time
                    let modified_at = tokio::fs::metadata(abs_path)
                        .await
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|t| {
                            let dt: chrono::DateTime<Utc> = t.into();
                            dt.to_rfc3339()
                        })
                        .unwrap_or_else(|| Utc::now().to_rfc3339());

                    match EntityService::upsert(db, vault_id, &rel_path, &fm, &modified_at, None)
                        .await
                    {
                        Ok(Some(_)) => {
                            indexed_count += 1;
                            visited_paths.push(rel_path);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            error!("Failed to upsert entity at {rel_path}: {e}");
                            error_count += 1;
                        }
                    }
                }
            }
        }

        // Remove stale entities (files deleted since last index)
        let known_paths = EntityService::get_indexed_paths(db, vault_id).await?;
        for stale_path in known_paths {
            if !visited_paths.contains(&stale_path) {
                debug!("Removing stale entity at {stale_path}");
                if let Err(e) = EntityService::remove(db, vault_id, &stale_path).await {
                    warn!("Failed to remove stale entity {stale_path}: {e}");
                }
            }
        }

        info!("Reindex pass 1 complete: {indexed_count} indexed, {error_count} errors");

        // --- Pass 2: sync relations ---
        let entities =
            crate::services::entity_service::EntityService::list_all_in_vault(db, vault_id).await?;
        let mut rel_errors = 0usize;
        for entity in &entities {
            if let Err(e) = RelationService::sync_from_entity(db, entity, None).await {
                warn!("Failed to sync relations for {}: {e}", entity.path);
                rel_errors += 1;
            }
        }

        let elapsed = Utc::now().signed_duration_since(start).num_milliseconds();

        info!(
            "Reindex complete for vault {vault_id}: {} entities, {} relations synced, {rel_errors} relation errors, {elapsed}ms",
            indexed_count,
            entities.len()
        );

        // Log to reindex_log
        let _ = sqlx::query(
            r#"
            INSERT INTO reindex_log (vault_id, started_at, finished_at, files_indexed, errors)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(vault_id)
        .bind(start.to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .bind(indexed_count as i64)
        .bind((error_count + rel_errors) as i64)
        .execute(db.pool())
        .await;

        Ok(indexed_count as i64)
    }

    /// Trigger re-sync of a single file's entity + relations.
    /// Called from the file-watcher event loop on Create/Modify events.
    pub async fn index_file(
        db: &Database,
        vault_id: &str,
        rel_path: &str,
        abs_path: &str,
    ) -> AppResult<()> {
        let content = match fs::read_to_string(abs_path).await {
            Ok(c) => c,
            Err(e) => {
                warn!("index_file: failed to read {abs_path}: {e}");
                return Ok(()); // Not a fatal error
            }
        };

        if let Some(fm) = EntityService::parse_frontmatter(&content) {
            if fm.get("codex_type").is_some() {
                let modified_at = tokio::fs::metadata(abs_path)
                    .await
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        let dt: chrono::DateTime<Utc> = t.into();
                        dt.to_rfc3339()
                    })
                    .unwrap_or_else(|| Utc::now().to_rfc3339());

                if let Ok(Some(entity)) =
                    EntityService::upsert(db, vault_id, rel_path, &fm, &modified_at, None).await
                {
                    RelationService::sync_from_entity(db, &entity, None).await?;
                }
                return Ok(());
            }
        }

        // File has no codex_type frontmatter — remove entity if it existed before
        // (user may have removed the codex_type key)
        EntityService::remove(db, vault_id, rel_path).await?;
        Ok(())
    }

    /// Remove entity + relations for a deleted file.
    /// Called from the file-watcher event loop on Delete events.
    pub async fn remove_file(db: &Database, vault_id: &str, rel_path: &str) -> AppResult<()> {
        EntityService::remove(db, vault_id, rel_path).await
    }
}

/// Recursively collect all `.md` files under `root`, skipping hidden directories
/// and the standard exclusion list.
async fn collect_md_files(root: &str) -> Vec<String> {
    let excluded_dirs = [".git", ".obsidian", ".trash", "node_modules", ".codex"];
    let mut results = Vec::new();
    collect_recursive(Path::new(root), &excluded_dirs, &mut results).await;
    results
}

fn collect_recursive<'a>(
    dir: &'a Path,
    excluded: &'a [&'a str],
    results: &'a mut Vec<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        let mut read_dir = match tokio::fs::read_dir(dir).await {
            Ok(r) => r,
            Err(_) => return,
        };
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || excluded.contains(&name) {
                    continue;
                }
                collect_recursive(&path, excluded, results).await;
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                results.push(path.to_string_lossy().into_owned());
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use tempfile::TempDir;

    // ── collect_md_files ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_collect_md_files_empty_dir() {
        let temp = TempDir::new().unwrap();
        let files = collect_md_files(temp.path().to_str().unwrap()).await;
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_collect_md_files_finds_md_files() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("note1.md"), "# Note 1").unwrap();
        std::fs::write(temp.path().join("note2.md"), "# Note 2").unwrap();
        std::fs::write(temp.path().join("image.png"), "binary").unwrap(); // non-md should be skipped

        let files = collect_md_files(temp.path().to_str().unwrap()).await;
        assert_eq!(files.len(), 2, "should find exactly 2 md files");
        assert!(files.iter().all(|f| f.ends_with(".md")));
    }

    #[tokio::test]
    async fn test_collect_md_files_recursion() {
        let temp = TempDir::new().unwrap();
        let sub = temp.path().join("Characters");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("hero.md"), "").unwrap();
        std::fs::write(temp.path().join("root.md"), "").unwrap();

        let files = collect_md_files(temp.path().to_str().unwrap()).await;
        assert_eq!(files.len(), 2, "should find md files in subdirectory");
    }

    #[tokio::test]
    async fn test_collect_md_files_skips_git_dir() {
        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(git_dir.join("HEAD"), "").unwrap();
        std::fs::write(git_dir.join("config.md"), "").unwrap(); // md inside .git — skip
        std::fs::write(temp.path().join("real.md"), "").unwrap();

        let files = collect_md_files(temp.path().to_str().unwrap()).await;
        assert_eq!(
            files.len(),
            1,
            "should only find real.md, not files in .git"
        );
    }

    #[tokio::test]
    async fn test_collect_md_files_skips_node_modules() {
        let temp = TempDir::new().unwrap();
        let nm = temp.path().join("node_modules");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(nm.join("package.md"), "").unwrap();
        std::fs::write(temp.path().join("actual.md"), "").unwrap();

        let files = collect_md_files(temp.path().to_str().unwrap()).await;
        assert_eq!(files.len(), 1);
    }

    #[tokio::test]
    async fn test_collect_md_files_skips_obsidian_dir() {
        let temp = TempDir::new().unwrap();
        let obs = temp.path().join(".obsidian");
        std::fs::create_dir_all(&obs).unwrap();
        std::fs::write(obs.join("workspace.md"), "").unwrap();
        std::fs::write(temp.path().join("note.md"), "").unwrap();

        let files = collect_md_files(temp.path().to_str().unwrap()).await;
        assert_eq!(files.len(), 1);
    }

    #[tokio::test]
    async fn test_collect_md_files_skips_trash_dir() {
        let temp = TempDir::new().unwrap();
        let trash = temp.path().join(".trash");
        std::fs::create_dir_all(&trash).unwrap();
        std::fs::write(trash.join("deleted.md"), "").unwrap();
        std::fs::write(temp.path().join("kept.md"), "").unwrap();

        let files = collect_md_files(temp.path().to_str().unwrap()).await;
        assert_eq!(files.len(), 1);
    }

    // ── ReindexService integration tests ──────────────────────────────────

    async fn setup_db(temp: &TempDir) -> Database {
        let db_path = temp.path().join("reindex-test.db");
        let db_url = format!("sqlite://{}", db_path.display());
        let db = Database::new(&db_url).await.expect("db should initialise");
        // Seed vault row required by FK constraint on entities.vault_id
        sqlx::query(
            "INSERT OR IGNORE INTO vaults (id, name, path, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("v1")
        .bind("Test Vault")
        .bind("/tmp/reindex-vault-v1")
        .bind("2024-01-01T00:00:00Z")
        .bind("2024-01-01T00:00:00Z")
        .execute(db.pool())
        .await
        .expect("vault seed failed");
        db
    }

    #[tokio::test]
    async fn test_reindex_vault_indexes_entity_file() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;
        let vault_dir = temp.path().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();

        let content = "---\ncodex_type: character\ncodex_plugin: worldbuilding\nfull_name: Hero\n---\n# Hero\n";
        std::fs::write(vault_dir.join("hero.md"), content).unwrap();

        ReindexService::reindex_vault(&db, "v1", vault_dir.to_str().unwrap())
            .await
            .unwrap();

        let entities = crate::services::entity_service::EntityService::list_all_in_vault(&db, "v1")
            .await
            .unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_type, "character");
        assert_eq!(entities[0].path, "hero.md");
    }

    #[tokio::test]
    async fn test_reindex_vault_skips_non_entity_files() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;
        let vault_dir = temp.path().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();

        // Normal markdown without codex_type
        std::fs::write(
            vault_dir.join("plain.md"),
            "# Just notes\n\nNo frontmatter.\n",
        )
        .unwrap();

        ReindexService::reindex_vault(&db, "v1", vault_dir.to_str().unwrap())
            .await
            .unwrap();

        let entities = crate::services::entity_service::EntityService::list_all_in_vault(&db, "v1")
            .await
            .unwrap();
        assert!(
            entities.is_empty(),
            "plain md with no codex_type should not be indexed"
        );
    }

    #[tokio::test]
    async fn test_reindex_removes_stale_entity() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;
        let vault_dir = temp.path().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();

        let hero_path = vault_dir.join("hero.md");
        std::fs::write(
            &hero_path,
            "---\ncodex_type: character\nfull_name: Hero\n---\n",
        )
        .unwrap();

        ReindexService::reindex_vault(&db, "v1", vault_dir.to_str().unwrap())
            .await
            .unwrap();
        // Verify entity was indexed
        let before = crate::services::entity_service::EntityService::list_all_in_vault(&db, "v1")
            .await
            .unwrap();
        assert_eq!(before.len(), 1);

        // Delete the file and reindex
        std::fs::remove_file(&hero_path).unwrap();
        ReindexService::reindex_vault(&db, "v1", vault_dir.to_str().unwrap())
            .await
            .unwrap();

        let after = crate::services::entity_service::EntityService::list_all_in_vault(&db, "v1")
            .await
            .unwrap();
        assert!(after.is_empty(), "stale entity should have been removed");
    }

    #[tokio::test]
    async fn test_index_file_indexes_entity() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;
        let vault_dir = temp.path().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();

        let abs_path = vault_dir.join("warrior.md");
        std::fs::write(
            &abs_path,
            "---\ncodex_type: character\ncodex_plugin: wb\n---\n",
        )
        .unwrap();

        ReindexService::index_file(&db, "v1", "warrior.md", abs_path.to_str().unwrap())
            .await
            .unwrap();

        let entity =
            crate::services::entity_service::EntityService::get_by_path(&db, "v1", "warrior.md")
                .await
                .unwrap();
        assert!(entity.is_some(), "index_file should create entity");
    }

    #[tokio::test]
    async fn test_index_file_removes_entity_when_codex_type_removed() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;
        let vault_dir = temp.path().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();

        let abs_path = vault_dir.join("warrior.md");
        std::fs::write(&abs_path, "---\ncodex_type: character\n---\n").unwrap();
        ReindexService::index_file(&db, "v1", "warrior.md", abs_path.to_str().unwrap())
            .await
            .unwrap();

        // Overwrite without codex_type
        std::fs::write(&abs_path, "# Just a note now\n").unwrap();
        ReindexService::index_file(&db, "v1", "warrior.md", abs_path.to_str().unwrap())
            .await
            .unwrap();

        let entity =
            crate::services::entity_service::EntityService::get_by_path(&db, "v1", "warrior.md")
                .await
                .unwrap();
        assert!(
            entity.is_none(),
            "entity should be removed when codex_type is absent"
        );
    }

    #[tokio::test]
    async fn test_remove_file_removes_entity() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;
        let vault_dir = temp.path().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();

        let abs_path = vault_dir.join("npc.md");
        std::fs::write(&abs_path, "---\ncodex_type: character\n---\n").unwrap();
        ReindexService::index_file(&db, "v1", "npc.md", abs_path.to_str().unwrap())
            .await
            .unwrap();

        ReindexService::remove_file(&db, "v1", "npc.md")
            .await
            .unwrap();

        let entity =
            crate::services::entity_service::EntityService::get_by_path(&db, "v1", "npc.md")
                .await
                .unwrap();
        assert!(entity.is_none(), "remove_file should delete the entity");
    }
}
