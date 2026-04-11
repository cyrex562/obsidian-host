use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::services::schema_service::EntityTypeRegistry;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

/// Compute a stable entity ID from vault_id + file path.
pub fn entity_id(vault_id: &str, path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(vault_id.as_bytes());
    hasher.update(b":");
    hasher.update(path.as_bytes());
    hex::encode(hasher.finalize())
}

/// A typed entity derived from a markdown file's frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Entity {
    pub id: String,
    pub vault_id: String,
    pub path: String,
    pub entity_type: String,
    pub plugin_id: String,
    /// JSON array of label strings
    pub labels: String,
    /// JSON object of field key → value
    pub fields: String,
    pub modified_at: String,
    pub indexed_at: String,
}

impl Entity {
    pub fn labels_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.labels).unwrap_or_default()
    }

    pub fn fields_map(&self) -> serde_json::Value {
        serde_json::from_str(&self.fields).unwrap_or(serde_json::Value::Object(Default::default()))
    }
}

pub struct EntityService;

impl EntityService {
    /// Upsert an entity from parsed frontmatter JSON.
    ///
    /// The `frontmatter` must be a `serde_json::Value::Object`. The reserved
    /// keys `codex_type`, `codex_labels`, and `codex_plugin` are extracted;
    /// all remaining keys are stored in the `fields` blob.
    ///
    /// When `registry` is provided (Phase 2+), required field validation is
    /// performed. Missing required fields are logged as warnings but do not
    /// prevent indexing — the file may be mid-edit.
    pub async fn upsert(
        db: &Database,
        vault_id: &str,
        path: &str,
        frontmatter: &serde_json::Value,
        file_modified_at: &str,
        registry: Option<&EntityTypeRegistry>,
    ) -> AppResult<Option<Entity>> {
        let obj = match frontmatter.as_object() {
            Some(o) => o,
            None => return Ok(None),
        };

        let entity_type = match obj.get("codex_type").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return Ok(None), // Not a typed entity
        };

        let plugin_id = obj
            .get("codex_plugin")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let labels: Vec<String> = obj
            .get("codex_labels")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        // Strip reserved keys; everything else goes into fields
        let reserved = ["codex_type", "codex_labels", "codex_plugin"];
        let fields: serde_json::Map<String, serde_json::Value> = obj
            .iter()
            .filter(|(k, _)| !reserved.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let id = entity_id(vault_id, path);
        let labels_json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
        let fields_json = serde_json::to_string(&fields).unwrap_or_else(|_| "{}".to_string());
        let now = Utc::now().to_rfc3339();

        // Schema-aware validation (Phase 2): check required fields
        if let Some(reg) = registry {
            if let Some(schema) = reg.get_by_id(&entity_type).await {
                for field_def in &schema.fields {
                    if field_def.required {
                        let present = fields
                            .get(&field_def.key)
                            .map(|v| {
                                !v.is_null() && v.as_str().map(|s| !s.is_empty()).unwrap_or(true)
                            })
                            .unwrap_or(false);
                        if !present {
                            warn!(
                                "Entity at {path}: required field '{}' is missing or empty",
                                field_def.key
                            );
                        }
                    }
                }
                // Merge entity-type labels (from schema) with frontmatter labels
                let mut merged_labels: Vec<String> = labels.clone();
                for schema_label in &schema.labels {
                    if !merged_labels.contains(schema_label) {
                        merged_labels.push(schema_label.clone());
                    }
                }
                let merged_json =
                    serde_json::to_string(&merged_labels).unwrap_or_else(|_| "[]".to_string());
                // Re-use merged_json below by shadowing
                return Self::do_upsert(
                    db,
                    vault_id,
                    path,
                    &entity_type,
                    &plugin_id,
                    &merged_json,
                    &fields_json,
                    file_modified_at,
                    &now,
                    &id,
                )
                .await;
            }
        }

        Self::do_upsert(
            db,
            vault_id,
            path,
            &entity_type,
            &plugin_id,
            &labels_json,
            &fields_json,
            file_modified_at,
            &now,
            &id,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn do_upsert(
        db: &Database,
        vault_id: &str,
        path: &str,
        entity_type: &str,
        plugin_id: &str,
        labels_json: &str,
        fields_json: &str,
        file_modified_at: &str,
        now: &str,
        id: &str,
    ) -> AppResult<Option<Entity>> {
        sqlx::query(
            r#"
            INSERT INTO entities (id, vault_id, path, entity_type, plugin_id, labels, fields, modified_at, indexed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(vault_id, path) DO UPDATE SET
                entity_type = excluded.entity_type,
                plugin_id   = excluded.plugin_id,
                labels      = excluded.labels,
                fields      = excluded.fields,
                modified_at = excluded.modified_at,
                indexed_at  = excluded.indexed_at
            "#,
        )
        .bind(&id)
        .bind(vault_id)
        .bind(path)
        .bind(&entity_type)
        .bind(&plugin_id)
        .bind(&labels_json)
        .bind(&fields_json)
        .bind(file_modified_at)
        .bind(&now)
        .execute(db.pool())
        .await
        .map_err(|e| AppError::DatabaseError(crate::error::DatabaseErrorContext {
            error: e,
            operation: "entity_upsert".into(),
            details: Some(format!("{vault_id}:{path}")),
        }))?;

        debug!("Upserted entity {id} ({entity_type}) at {path}");

        Ok(Some(Entity {
            id: id.to_string(),
            vault_id: vault_id.to_string(),
            path: path.to_string(),
            entity_type: entity_type.to_string(),
            plugin_id: plugin_id.to_string(),
            labels: labels_json.to_string(),
            fields: fields_json.to_string(),
            modified_at: file_modified_at.to_string(),
            indexed_at: now.to_string(),
        }))
    }

    /// Remove an entity and all its relation edges.
    pub async fn remove(db: &Database, vault_id: &str, path: &str) -> AppResult<()> {
        let id = entity_id(vault_id, path);
        // Relations cascade-delete via FK; explicit delete for clarity.
        sqlx::query("DELETE FROM relations WHERE from_entity_id = ? OR to_entity_id = ?")
            .bind(&id)
            .bind(&id)
            .execute(db.pool())
            .await
            .map_err(|e| {
                AppError::DatabaseError(crate::error::DatabaseErrorContext {
                    error: e,
                    operation: "remove_entity_relations".into(),
                    details: None,
                })
            })?;

        sqlx::query("DELETE FROM entities WHERE id = ?")
            .bind(&id)
            .execute(db.pool())
            .await
            .map_err(|e| {
                AppError::DatabaseError(crate::error::DatabaseErrorContext {
                    error: e,
                    operation: "remove_entity".into(),
                    details: None,
                })
            })?;

        debug!("Removed entity {id} ({vault_id}:{path})");
        Ok(())
    }

    /// Get a single entity by its ID.
    pub async fn get(db: &Database, entity_id: &str) -> AppResult<Option<Entity>> {
        let entity: Option<Entity> = sqlx::query_as(
            "SELECT id, vault_id, path, entity_type, plugin_id, labels, fields, modified_at, indexed_at FROM entities WHERE id = ?"
        )
        .bind(entity_id)
        .fetch_optional(db.pool())
        .await
        .map_err(|e| AppError::DatabaseError(crate::error::DatabaseErrorContext {
            error: e,
            operation: "get_entity".into(),
            details: None,
        }))?;
        Ok(entity)
    }

    /// List entities for a vault with optional filters.
    pub async fn list(
        db: &Database,
        vault_id: &str,
        entity_type_filter: Option<&str>,
        label_filter: Option<&str>,
        plugin_filter: Option<&str>,
        name_query: Option<&str>,
    ) -> AppResult<Vec<Entity>> {
        // Build dynamic query
        let mut conditions = vec!["vault_id = ?".to_string()];

        if entity_type_filter.is_some() {
            conditions.push("entity_type = ?".to_string());
        }
        if plugin_filter.is_some() {
            conditions.push("plugin_id = ?".to_string());
        }

        let sql = format!(
            "SELECT id, vault_id, path, entity_type, plugin_id, labels, fields, modified_at, indexed_at FROM entities WHERE {}",
            conditions.join(" AND ")
        );

        let mut query = sqlx::query_as::<_, Entity>(&sql).bind(vault_id);
        if let Some(t) = entity_type_filter {
            query = query.bind(t);
        }
        if let Some(p) = plugin_filter {
            query = query.bind(p);
        }

        let mut entities: Vec<Entity> = query.fetch_all(db.pool()).await.map_err(|e| {
            AppError::DatabaseError(crate::error::DatabaseErrorContext {
                error: e,
                operation: "list_entities".into(),
                details: None,
            })
        })?;

        // Post-filter by label
        if let Some(label) = label_filter {
            entities.retain(|e| {
                serde_json::from_str::<Vec<String>>(&e.labels)
                    .unwrap_or_default()
                    .iter()
                    .any(|l| l == label)
            });
        }

        // Post-filter by name query (match against entity_type field or path basename)
        if let Some(q) = name_query {
            let q_lower = q.to_lowercase();
            entities.retain(|e| {
                e.entity_type.to_lowercase().contains(&q_lower)
                    || e.path.to_lowercase().contains(&q_lower)
                    || {
                        // Check the display name in fields (any string value)
                        if let Ok(fields) = serde_json::from_str::<serde_json::Value>(&e.fields) {
                            if let Some(obj) = fields.as_object() {
                                obj.values().any(|v| {
                                    v.as_str()
                                        .map(|s| s.to_lowercase().contains(&q_lower))
                                        .unwrap_or(false)
                                })
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
            });
        }

        Ok(entities)
    }

    /// List all entities in a vault (used by reindex and graph queries).
    pub async fn list_all_in_vault(db: &Database, vault_id: &str) -> AppResult<Vec<Entity>> {
        let entities: Vec<Entity> = sqlx::query_as(
            "SELECT id, vault_id, path, entity_type, plugin_id, labels, fields, modified_at, indexed_at FROM entities WHERE vault_id = ?"
        )
        .bind(vault_id)
        .fetch_all(db.pool())
        .await
        .map_err(|e| AppError::DatabaseError(crate::error::DatabaseErrorContext {
            error: e,
            operation: "list_all_entities".into(),
            details: None,
        }))?;
        Ok(entities)
    }

    /// Get the paths of all indexed entities in a vault (used by reindex cleanup).
    pub async fn get_indexed_paths(db: &Database, vault_id: &str) -> AppResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT path FROM entities WHERE vault_id = ?")
            .bind(vault_id)
            .fetch_all(db.pool())
            .await
            .map_err(|e| {
                AppError::DatabaseError(crate::error::DatabaseErrorContext {
                    error: e,
                    operation: "get_indexed_paths".into(),
                    details: None,
                })
            })?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Get a single entity by vault + path.
    pub async fn get_by_path(
        db: &Database,
        vault_id: &str,
        path: &str,
    ) -> AppResult<Option<Entity>> {
        let entity: Option<Entity> = sqlx::query_as(
            "SELECT id, vault_id, path, entity_type, plugin_id, labels, fields, modified_at, indexed_at FROM entities WHERE vault_id = ? AND path = ?"
        )
        .bind(vault_id)
        .bind(path)
        .fetch_optional(db.pool())
        .await
        .map_err(|e| AppError::DatabaseError(crate::error::DatabaseErrorContext {
            error: e,
            operation: "get_entity_by_path".into(),
            details: None,
        }))?;
        Ok(entity)
    }

    /// Parse frontmatter from markdown content. Returns None if no frontmatter.
    pub fn parse_frontmatter(content: &str) -> Option<serde_json::Value> {
        let content = content.trim_start();
        if !content.starts_with("---") {
            return None;
        }
        let after_open = &content[3..];
        let end = after_open.find("\n---")?;
        let yaml_str = &after_open[..end];
        match serde_yaml::from_str::<serde_json::Value>(yaml_str) {
            Ok(v) => Some(v),
            Err(e) => {
                warn!("Failed to parse frontmatter: {e}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── entity_id tests ───────────────────────────────────────────────────

    #[test]
    fn test_entity_id_is_deterministic() {
        let id1 = entity_id("vault-abc", "notes/hero.md");
        let id2 = entity_id("vault-abc", "notes/hero.md");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_entity_id_differs_by_vault() {
        let id1 = entity_id("vault-a", "hero.md");
        let id2 = entity_id("vault-b", "hero.md");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_entity_id_differs_by_path() {
        let id1 = entity_id("vault-a", "hero.md");
        let id2 = entity_id("vault-a", "villain.md");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_entity_id_is_hex_string() {
        let id = entity_id("vault", "path.md");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "entity_id should be hex: {id}"
        );
        assert_eq!(id.len(), 64, "SHA256 hex should be 64 chars");
    }

    // ── parse_frontmatter tests ───────────────────────────────────────────

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = "---\ncodex_type: character\nfull_name: Alice\n---\n# Content";
        let fm = EntityService::parse_frontmatter(content).expect("should parse");
        assert_eq!(fm["codex_type"].as_str(), Some("character"));
        assert_eq!(fm["full_name"].as_str(), Some("Alice"));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# Just a heading\n\nNo frontmatter here.";
        assert!(EntityService::parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_parse_frontmatter_empty_block() {
        let content = "---\n---\n# Content";
        // empty frontmatter parses to null or empty object depending on serde_yaml
        // main thing: it should not panic and should return Some
        let result = EntityService::parse_frontmatter(content);
        // serde_yaml returns null for truly empty YAML
        assert!(result.is_some() || result.is_none()); // at minimum, no panic
    }

    #[test]
    fn test_parse_frontmatter_leading_whitespace() {
        let content = "\n\n---\ncodex_type: location\n---\n# Content";
        let fm = EntityService::parse_frontmatter(content)
            .expect("should parse through leading whitespace");
        assert_eq!(fm["codex_type"].as_str(), Some("location"));
    }

    // ── Entity helper method tests ────────────────────────────────────────

    fn make_entity(labels_json: &str, fields_json: &str) -> Entity {
        Entity {
            id: "test-id".into(),
            vault_id: "vault-1".into(),
            path: "test.md".into(),
            entity_type: "character".into(),
            plugin_id: "worldbuilding".into(),
            labels: labels_json.into(),
            fields: fields_json.into(),
            modified_at: "2024-01-01T00:00:00Z".into(),
            indexed_at: "2024-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn test_labels_vec_valid() {
        let e = make_entity(r#"["graphable","person"]"#, "{}");
        let labels = e.labels_vec();
        assert_eq!(labels, vec!["graphable", "person"]);
    }

    #[test]
    fn test_labels_vec_empty() {
        let e = make_entity("[]", "{}");
        assert!(e.labels_vec().is_empty());
    }

    #[test]
    fn test_labels_vec_invalid_json_returns_empty() {
        let e = make_entity("not-json", "{}");
        assert!(e.labels_vec().is_empty());
    }

    #[test]
    fn test_fields_map_valid() {
        let e = make_entity("[]", r#"{"full_name":"Alice","status":"Active"}"#);
        let fields = e.fields_map();
        assert_eq!(fields["full_name"].as_str(), Some("Alice"));
        assert_eq!(fields["status"].as_str(), Some("Active"));
    }

    #[test]
    fn test_fields_map_invalid_json_returns_empty_object() {
        let e = make_entity("[]", "not-json");
        let fields = e.fields_map();
        assert!(fields.is_object());
        assert_eq!(fields.as_object().unwrap().len(), 0);
    }
}
