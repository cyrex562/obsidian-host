use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::services::entity_service::{Entity, EntityService};
use crate::services::schema_service::RelationTypeRegistry;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Relation {
    pub id: String,
    pub vault_id: String,
    pub from_entity_id: String,
    pub to_entity_id: String,
    pub relation_type: String,
    pub direction: String,
    pub metadata: Option<String>,
    pub source: String,
    pub source_field: Option<String>,
    pub created_at: String,
}

pub struct RelationService;

impl RelationService {
    /// Derive relation edges from an entity's `entity_ref` fields.
    ///
    /// When `registry` is provided, the inverse label is looked up from the
    /// registered relation type schema. Otherwise falls back to the Phase 1
    /// generic `"inverse_of_<field>"` name.
    pub async fn sync_from_entity(
        db: &Database,
        entity: &Entity,
        registry: Option<&RelationTypeRegistry>,
    ) -> AppResult<()> {
        // Remove all existing field-derived relations for this entity so we
        // start fresh (avoids stale edges after a field value changes).
        // This includes both outgoing forward edges AND the mirror inverse edges
        // that point back to this entity from its targets.
        sqlx::query(
            "DELETE FROM relations WHERE source = 'field' AND \
             (from_entity_id = ? OR (to_entity_id = ? AND direction = 'inverse'))",
        )
        .bind(&entity.id)
        .bind(&entity.id)
        .execute(db.pool())
        .await
        .map_err(|e| {
            AppError::DatabaseError(crate::error::DatabaseErrorContext {
                error: e,
                operation: "clear_field_relations".into(),
                details: None,
            })
        })?;

        let fields = entity.fields_map();
        let obj = match fields.as_object() {
            Some(o) => o,
            None => return Ok(()),
        };

        for (field_key, field_value) in obj {
            // Handle both scalar and list entity refs
            let ref_values: Vec<&str> = match field_value {
                serde_json::Value::String(s) => vec![s.as_str()],
                serde_json::Value::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect(),
                _ => continue,
            };

            for ref_val in ref_values {
                let title = extract_wiki_title(ref_val);
                if title.is_none() {
                    continue;
                }
                let title = title.unwrap();

                // Try to resolve by basename match
                let target = Self::resolve_target(db, &entity.vault_id, title).await;
                match target {
                    Ok(Some(target_entity)) => {
                        // Determine the inverse label using registry if available
                        let inverse_type = if let Some(reg) = registry {
                            reg.find_by_name(field_key)
                                .await
                                .and_then(|rt| rt.inverse_label)
                                .unwrap_or_else(|| format!("inverse_of_{field_key}"))
                        } else {
                            format!("inverse_of_{field_key}")
                        };

                        // Create forward edge
                        Self::insert_relation(
                            db,
                            &entity.vault_id,
                            &entity.id,
                            &target_entity.id,
                            field_key,
                            "forward",
                            field_key,
                        )
                        .await?;

                        // Create inverse edge
                        Self::insert_relation(
                            db,
                            &entity.vault_id,
                            &target_entity.id,
                            &entity.id,
                            &inverse_type,
                            "inverse",
                            field_key,
                        )
                        .await?;

                        debug!(
                            "Created relation {} → {} ({})",
                            entity.id, target_entity.id, field_key
                        );
                    }
                    Ok(None) => {
                        debug!(
                            "Unresolved entity_ref in {}: [[{title}]] — skipping",
                            entity.path
                        );
                    }
                    Err(e) => {
                        warn!("Error resolving entity_ref [[{title}]]: {e}");
                    }
                }
            }
        }

        Ok(())
    }

    /// Resolve a `[[Title]]` or `[[Title|Alias]]` reference to an entity by
    /// matching the path's file stem (case-insensitive) within the vault.
    ///
    /// Returns `None` gracefully if the target is not yet indexed.
    pub async fn resolve_target(
        db: &Database,
        vault_id: &str,
        title: &str,
    ) -> AppResult<Option<Entity>> {
        // Try exact stem match first, then case-insensitive
        let all = EntityService::list_all_in_vault(db, vault_id).await?;
        let title_lower = title.to_lowercase();
        let found = all.into_iter().find(|e| {
            let stem = std::path::Path::new(&e.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            stem.to_lowercase() == title_lower
        });
        Ok(found)
    }

    async fn insert_relation(
        db: &Database,
        vault_id: &str,
        from_id: &str,
        to_id: &str,
        relation_type: &str,
        direction: &str,
        source_field: &str,
    ) -> AppResult<()> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO relations
            (id, vault_id, from_entity_id, to_entity_id, relation_type, direction, metadata, source, source_field, created_at)
            VALUES (?, ?, ?, ?, ?, ?, NULL, 'field', ?, ?)
            "#,
        )
        .bind(id)
        .bind(vault_id)
        .bind(from_id)
        .bind(to_id)
        .bind(relation_type)
        .bind(direction)
        .bind(source_field)
        .bind(now)
        .execute(db.pool())
        .await
        .map_err(|e| AppError::DatabaseError(crate::error::DatabaseErrorContext {
            error: e,
            operation: "insert_relation".into(),
            details: None,
        }))?;
        Ok(())
    }

    /// Get all relations connected to an entity (both directions).
    pub async fn get_for_entity(db: &Database, entity_id: &str) -> AppResult<Vec<Relation>> {
        let relations: Vec<Relation> = sqlx::query_as(
            r#"
            SELECT id, vault_id, from_entity_id, to_entity_id, relation_type, direction,
                   metadata, source, source_field, created_at
            FROM relations
            WHERE from_entity_id = ? OR to_entity_id = ?
            "#,
        )
        .bind(entity_id)
        .bind(entity_id)
        .fetch_all(db.pool())
        .await
        .map_err(|e| {
            AppError::DatabaseError(crate::error::DatabaseErrorContext {
                error: e,
                operation: "get_relations_for_entity".into(),
                details: None,
            })
        })?;
        Ok(relations)
    }

    /// Get all relations in a vault.
    pub async fn list_for_vault(db: &Database, vault_id: &str) -> AppResult<Vec<Relation>> {
        let relations: Vec<Relation> = sqlx::query_as(
            r#"
            SELECT id, vault_id, from_entity_id, to_entity_id, relation_type, direction,
                   metadata, source, source_field, created_at
            FROM relations WHERE vault_id = ?
            "#,
        )
        .bind(vault_id)
        .fetch_all(db.pool())
        .await
        .map_err(|e| {
            AppError::DatabaseError(crate::error::DatabaseErrorContext {
                error: e,
                operation: "list_vault_relations".into(),
                details: None,
            })
        })?;
        Ok(relations)
    }

    /// Update relation metadata (used by structural editor sub-form).
    pub async fn update_metadata(
        db: &Database,
        relation_id: &str,
        metadata: &serde_json::Value,
    ) -> AppResult<()> {
        let metadata_json = serde_json::to_string(metadata).map_err(|e| {
            AppError::InternalError(format!("Failed to serialize relation metadata: {e}"))
        })?;
        sqlx::query("UPDATE relations SET metadata = ? WHERE id = ?")
            .bind(metadata_json)
            .bind(relation_id)
            .execute(db.pool())
            .await
            .map_err(|e| {
                AppError::DatabaseError(crate::error::DatabaseErrorContext {
                    error: e,
                    operation: "update_relation_metadata".into(),
                    details: None,
                })
            })?;
        Ok(())
    }
}

/// Extract the title from a wiki-link string like `[[Title]]` or `[[Title|Alias]]`.
/// Returns `None` if the string is not a wiki-link.
fn extract_wiki_title(s: &str) -> Option<&str> {
    let s = s.trim();
    if !s.starts_with("[[") || !s.ends_with("]]") {
        return None;
    }
    let inner = &s[2..s.len() - 2];
    // Strip alias: [[Title|Alias]] → "Title"
    Some(inner.split('|').next().unwrap_or(inner).trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::services::entity_service::EntityService;
    use tempfile::TempDir;

    // ── extract_wiki_title ────────────────────────────────────────────────

    #[test]
    fn test_extract_wiki_title_simple() {
        assert_eq!(extract_wiki_title("[[Alice]]"), Some("Alice"));
    }

    #[test]
    fn test_extract_wiki_title_with_alias() {
        assert_eq!(extract_wiki_title("[[Alice|The Hero]]"), Some("Alice"));
    }

    #[test]
    fn test_extract_wiki_title_with_spaces() {
        assert_eq!(extract_wiki_title("[[  Lyra  ]]"), Some("Lyra"));
    }

    #[test]
    fn test_extract_wiki_title_outer_whitespace_trimmed() {
        assert_eq!(
            extract_wiki_title("  [[Castle Keep]]  "),
            Some("Castle Keep")
        );
    }

    #[test]
    fn test_extract_wiki_title_not_wiki_link_plain() {
        assert!(extract_wiki_title("just a string").is_none());
    }

    #[test]
    fn test_extract_wiki_title_not_wiki_link_single_bracket() {
        assert!(extract_wiki_title("[single]").is_none());
    }

    #[test]
    fn test_extract_wiki_title_empty_string() {
        assert!(extract_wiki_title("").is_none());
    }

    #[test]
    fn test_extract_wiki_title_only_brackets() {
        // [[]] — empty title; inner is empty, but still valid link
        assert_eq!(extract_wiki_title("[[]]"), Some(""));
    }

    #[test]
    fn test_extract_wiki_title_alias_only_keeps_title_part() {
        // Multiple pipes: [[Title|Alias1|Extra]] → only first segment
        assert_eq!(extract_wiki_title("[[Hero|The Hero|extra]]"), Some("Hero"));
    }

    // ── DB-backed tests ────────────────────────────────────────────────────

    async fn setup_db(temp: &TempDir) -> Database {
        let db_path = temp.path().join("rel-test.db");
        let db_url = format!("sqlite://{}", db_path.display());
        let db = Database::new(&db_url).await.expect("db should initialise");
        // Seed vault rows required by FK constraint on entities.vault_id
        for (id, path) in &[("vault-1", "/tmp/rel-vault-1"), ("v1", "/tmp/rel-v1")] {
            sqlx::query(
                "INSERT OR IGNORE INTO vaults (id, name, path, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind("Test Vault")
            .bind(path)
            .bind("2024-01-01T00:00:00Z")
            .bind("2024-01-01T00:00:00Z")
            .execute(db.pool())
            .await
            .expect("vault seed failed");
        }
        db
    }

    fn make_frontmatter(entity_type: &str, fields: serde_json::Value) -> serde_json::Value {
        let mut obj = fields.as_object().cloned().unwrap_or_default();
        obj.insert(
            "codex_type".into(),
            serde_json::Value::String(entity_type.into()),
        );
        obj.insert(
            "codex_plugin".into(),
            serde_json::Value::String("worldbuilding".into()),
        );
        serde_json::Value::Object(obj)
    }

    #[tokio::test]
    async fn test_resolve_target_returns_none_for_unknown_title() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;
        let result = RelationService::resolve_target(&db, "vault-1", "NonExistent").await;
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_resolve_target_finds_entity_by_stem() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        let fm = make_frontmatter("character", serde_json::json!({ "full_name": "Alice" }));
        EntityService::upsert(
            &db,
            "vault-1",
            "alice.md",
            &fm,
            "2024-01-01T00:00:00Z",
            None,
        )
        .await
        .unwrap();

        let result = RelationService::resolve_target(&db, "vault-1", "alice")
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().path, "alice.md");
    }

    #[tokio::test]
    async fn test_resolve_target_case_insensitive() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        let fm = make_frontmatter("character", serde_json::json!({}));
        EntityService::upsert(
            &db,
            "vault-1",
            "CastleKeep.md",
            &fm,
            "2024-01-01T00:00:00Z",
            None,
        )
        .await
        .unwrap();

        let result = RelationService::resolve_target(&db, "vault-1", "castlekeep")
            .await
            .unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_sync_from_entity_creates_no_relations_when_no_refs() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        let fm = make_frontmatter("character", serde_json::json!({ "full_name": "Alice" }));
        let entity =
            EntityService::upsert(&db, "v1", "alice.md", &fm, "2024-01-01T00:00:00Z", None)
                .await
                .unwrap()
                .unwrap();

        RelationService::sync_from_entity(&db, &entity, None)
            .await
            .unwrap();

        let relations = RelationService::get_for_entity(&db, &entity.id)
            .await
            .unwrap();
        assert!(relations.is_empty());
    }

    #[tokio::test]
    async fn test_sync_from_entity_creates_forward_and_inverse_relation() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        // Index both entities
        let alice_fm = make_frontmatter("character", serde_json::json!({ "faction": "[[Order]]" }));
        let alice = EntityService::upsert(
            &db,
            "v1",
            "alice.md",
            &alice_fm,
            "2024-01-01T00:00:00Z",
            None,
        )
        .await
        .unwrap()
        .unwrap();

        let order_fm = make_frontmatter("faction", serde_json::json!({}));
        let order = EntityService::upsert(
            &db,
            "v1",
            "Order.md",
            &order_fm,
            "2024-01-01T00:00:00Z",
            None,
        )
        .await
        .unwrap()
        .unwrap();

        RelationService::sync_from_entity(&db, &alice, None)
            .await
            .unwrap();

        // Forward edge: alice → order
        let alice_relations = RelationService::get_for_entity(&db, &alice.id)
            .await
            .unwrap();
        assert!(
            !alice_relations.is_empty(),
            "alice should have at least one relation"
        );

        // Inverse edge visible from order's perspective
        let order_relations = RelationService::get_for_entity(&db, &order.id)
            .await
            .unwrap();
        assert!(
            !order_relations.is_empty(),
            "order should have an inverse relation"
        );

        let forward = alice_relations.iter().find(|r| r.direction == "forward");
        assert!(forward.is_some(), "should have a forward relation");
        assert_eq!(forward.unwrap().from_entity_id, alice.id);
        assert_eq!(forward.unwrap().to_entity_id, order.id);
    }

    #[tokio::test]
    async fn test_sync_from_entity_clears_stale_relations_on_resync() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        let alice_fm = make_frontmatter("character", serde_json::json!({ "faction": "[[Order]]" }));
        let alice = EntityService::upsert(
            &db,
            "v1",
            "alice.md",
            &alice_fm,
            "2024-01-01T00:00:00Z",
            None,
        )
        .await
        .unwrap()
        .unwrap();

        EntityService::upsert(
            &db,
            "v1",
            "Order.md",
            &make_frontmatter("faction", serde_json::json!({})),
            "2024-01-01T00:00:00Z",
            None,
        )
        .await
        .unwrap();

        // First sync — creates relations
        RelationService::sync_from_entity(&db, &alice, None)
            .await
            .unwrap();
        let count_first = RelationService::get_for_entity(&db, &alice.id)
            .await
            .unwrap()
            .len();
        assert!(count_first > 0);

        // Re-sync — should not double-count
        RelationService::sync_from_entity(&db, &alice, None)
            .await
            .unwrap();
        let count_second = RelationService::get_for_entity(&db, &alice.id)
            .await
            .unwrap()
            .len();
        assert_eq!(
            count_first, count_second,
            "re-sync should not create duplicate relations"
        );
    }

    #[tokio::test]
    async fn test_update_metadata_stores_json() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        // Create two entities and a relation between them
        let fm_a = make_frontmatter("character", serde_json::json!({ "friend": "[[Bob]]" }));
        let alice =
            EntityService::upsert(&db, "v1", "alice.md", &fm_a, "2024-01-01T00:00:00Z", None)
                .await
                .unwrap()
                .unwrap();

        EntityService::upsert(
            &db,
            "v1",
            "bob.md",
            &make_frontmatter("character", serde_json::json!({})),
            "2024-01-01T00:00:00Z",
            None,
        )
        .await
        .unwrap();

        RelationService::sync_from_entity(&db, &alice, None)
            .await
            .unwrap();

        let relations = RelationService::get_for_entity(&db, &alice.id)
            .await
            .unwrap();
        let rel = relations
            .iter()
            .find(|r| r.direction == "forward")
            .expect("forward relation");

        let metadata = serde_json::json!({ "relationship": "Friend" });
        RelationService::update_metadata(&db, &rel.id, &metadata)
            .await
            .unwrap();

        // Re-fetch and check
        let updated = RelationService::get_for_entity(&db, &alice.id)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id == rel.id)
            .unwrap();
        let stored: serde_json::Value =
            serde_json::from_str(updated.metadata.as_deref().unwrap_or("{}")).unwrap();
        assert_eq!(stored["relationship"].as_str(), Some("Friend"));
    }
}
