use crate::db::Database;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Core reserved labels that ship with Codex. Plugins may not redefine these.
pub const CORE_LABELS: &[(&str, &str)] = &[
    ("graphable", "Entity appears in graph views"),
    ("person", "A human or human-like character"),
    ("organization", "A group, faction, institution, or company"),
    ("place", "A physical or conceptual location"),
    ("event", "A point or span in time"),
    ("object", "A physical artefact or item"),
    ("concept", "An abstract idea, technology, or system"),
    ("creature", "A non-human living entity"),
];

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Label {
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub plugin_id: Option<String>,
}

pub struct LabelService;

impl LabelService {
    /// Seed the 8 core reserved labels on startup. Uses INSERT OR IGNORE so
    /// this is safe to call on every startup.
    pub async fn seed_core_labels(db: &Database) -> AppResult<()> {
        for (name, description) in CORE_LABELS {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO labels (name, description, source, plugin_id)
                VALUES (?, ?, 'core', NULL)
                "#,
            )
            .bind(name)
            .bind(description)
            .execute(db.pool())
            .await
            .map_err(|e| {
                AppError::DatabaseError(crate::error::DatabaseErrorContext {
                    error: e,
                    operation: "seed_core_labels".into(),
                    details: Some(format!("label: {name}")),
                })
            })?;
        }
        info!("Seeded {} core labels", CORE_LABELS.len());
        Ok(())
    }

    /// Register a plugin-declared label. Returns a `Conflict` error if the
    /// label name is already owned by a *different* plugin (or by core).
    pub async fn register(
        db: &Database,
        name: &str,
        description: Option<&str>,
        plugin_id: &str,
    ) -> AppResult<()> {
        // Check for existing registration
        let existing: Option<Label> = sqlx::query_as(
            "SELECT name, description, source, plugin_id FROM labels WHERE name = ?",
        )
        .bind(name)
        .fetch_optional(db.pool())
        .await
        .map_err(|e| {
            AppError::DatabaseError(crate::error::DatabaseErrorContext {
                error: e,
                operation: "register_label_check".into(),
                details: None,
            })
        })?;

        if let Some(existing) = existing {
            let owner = existing.plugin_id.as_deref().unwrap_or("<core>");
            if owner != plugin_id {
                return Err(AppError::Conflict(format!(
                    "Label '{name}' is already registered by '{owner}'"
                )));
            }
            // Same plugin re-registering — idempotent, nothing to do.
            return Ok(());
        }

        sqlx::query(
            r#"
            INSERT INTO labels (name, description, source, plugin_id)
            VALUES (?, ?, 'plugin', ?)
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(plugin_id)
        .execute(db.pool())
        .await
        .map_err(|e| {
            AppError::DatabaseError(crate::error::DatabaseErrorContext {
                error: e,
                operation: "register_label".into(),
                details: Some(format!("label: {name}")),
            })
        })?;

        Ok(())
    }

    /// Remove all labels registered by the given plugin.
    pub async fn remove_plugin_labels(db: &Database, plugin_id: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM labels WHERE plugin_id = ?")
            .bind(plugin_id)
            .execute(db.pool())
            .await
            .map_err(|e| {
                AppError::DatabaseError(crate::error::DatabaseErrorContext {
                    error: e,
                    operation: "remove_plugin_labels".into(),
                    details: None,
                })
            })?;
        Ok(())
    }

    /// List all registered labels (core + plugin).
    pub async fn list(db: &Database) -> AppResult<Vec<Label>> {
        let labels: Vec<Label> = sqlx::query_as(
            "SELECT name, description, source, plugin_id FROM labels ORDER BY source, name",
        )
        .fetch_all(db.pool())
        .await
        .map_err(|e| {
            AppError::DatabaseError(crate::error::DatabaseErrorContext {
                error: e,
                operation: "list_labels".into(),
                details: None,
            })
        })?;
        Ok(labels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_db(temp: &TempDir) -> Database {
        let db_path = temp.path().join("label-test.db");
        let db_url = format!("sqlite://{}", db_path.display());
        Database::new(&db_url).await.expect("db should initialise")
    }

    // ── CORE_LABELS constant ──────────────────────────────────────────────

    #[test]
    fn test_core_labels_count() {
        assert_eq!(CORE_LABELS.len(), 8, "expected exactly 8 core labels");
    }

    #[test]
    fn test_core_labels_contains_expected_names() {
        let names: Vec<&str> = CORE_LABELS.iter().map(|(n, _)| *n).collect();
        for expected in [
            "graphable",
            "person",
            "organization",
            "place",
            "event",
            "object",
            "concept",
            "creature",
        ] {
            assert!(names.contains(&expected), "missing core label: {expected}");
        }
    }

    #[test]
    fn test_core_labels_all_have_descriptions() {
        for (name, desc) in CORE_LABELS {
            assert!(
                !desc.is_empty(),
                "core label '{name}' should have a description"
            );
        }
    }

    // ── seed_core_labels ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_seed_core_labels_creates_all_labels() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::seed_core_labels(&db).await.unwrap();

        let labels = LabelService::list(&db).await.unwrap();
        assert_eq!(
            labels.len(),
            CORE_LABELS.len(),
            "should have all core labels in DB"
        );
    }

    #[tokio::test]
    async fn test_seed_core_labels_idempotent() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::seed_core_labels(&db).await.unwrap();
        LabelService::seed_core_labels(&db).await.unwrap(); // second call should not error or duplicate

        let labels = LabelService::list(&db).await.unwrap();
        assert_eq!(
            labels.len(),
            CORE_LABELS.len(),
            "no duplicates on double-seed"
        );
    }

    #[tokio::test]
    async fn test_seed_core_labels_source_is_core() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::seed_core_labels(&db).await.unwrap();

        let labels = LabelService::list(&db).await.unwrap();
        for label in &labels {
            assert_eq!(label.source, "core");
            assert!(label.plugin_id.is_none());
        }
    }

    // ── register ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_register_plugin_label() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::register(
            &db,
            "undead",
            Some("A dead but animated entity"),
            "com.plugin.undead",
        )
        .await
        .unwrap();

        let labels = LabelService::list(&db).await.unwrap();
        let label = labels
            .iter()
            .find(|l| l.name == "undead")
            .expect("should have undead label");
        assert_eq!(label.source, "plugin");
        assert_eq!(label.plugin_id.as_deref(), Some("com.plugin.undead"));
    }

    #[tokio::test]
    async fn test_register_same_plugin_idempotent() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::register(&db, "custom", None, "plugin-a")
            .await
            .unwrap();
        // Re-registering from same plugin should be a no-op (not an error)
        LabelService::register(&db, "custom", None, "plugin-a")
            .await
            .unwrap();

        let labels = LabelService::list(&db).await.unwrap();
        let count = labels.iter().filter(|l| l.name == "custom").count();
        assert_eq!(count, 1, "no duplicate should be created");
    }

    #[tokio::test]
    async fn test_register_conflict_with_different_plugin() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::register(&db, "custom", None, "plugin-a")
            .await
            .unwrap();
        let result = LabelService::register(&db, "custom", None, "plugin-b").await;

        assert!(result.is_err(), "second plugin should get a Conflict error");
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("already registered"),
            "error message should mention conflict: {err_str}"
        );
    }

    #[tokio::test]
    async fn test_register_conflict_with_core_label() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::seed_core_labels(&db).await.unwrap();

        let result = LabelService::register(&db, "graphable", None, "some-plugin").await;
        assert!(result.is_err(), "plugin cannot override core label");
    }

    // ── remove_plugin_labels ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_remove_plugin_labels() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::register(&db, "label-a", None, "plugin-x")
            .await
            .unwrap();
        LabelService::register(&db, "label-b", None, "plugin-x")
            .await
            .unwrap();
        LabelService::register(&db, "label-c", None, "plugin-y")
            .await
            .unwrap();

        LabelService::remove_plugin_labels(&db, "plugin-x")
            .await
            .unwrap();

        let labels = LabelService::list(&db).await.unwrap();
        assert!(
            !labels.iter().any(|l| l.name == "label-a"),
            "label-a should be removed"
        );
        assert!(
            !labels.iter().any(|l| l.name == "label-b"),
            "label-b should be removed"
        );
        assert!(
            labels.iter().any(|l| l.name == "label-c"),
            "label-c (plugin-y) should remain"
        );
    }

    #[tokio::test]
    async fn test_remove_plugin_labels_does_not_affect_core() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::seed_core_labels(&db).await.unwrap();
        LabelService::remove_plugin_labels(&db, "some-plugin")
            .await
            .unwrap();

        let labels = LabelService::list(&db).await.unwrap();
        assert_eq!(
            labels.len(),
            CORE_LABELS.len(),
            "core labels should be untouched"
        );
    }

    // ── list ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_returns_empty_on_fresh_db() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;
        let labels = LabelService::list(&db).await.unwrap();
        assert!(labels.is_empty());
    }

    #[tokio::test]
    async fn test_list_includes_both_core_and_plugin_labels() {
        let temp = TempDir::new().unwrap();
        let db = setup_db(&temp).await;

        LabelService::seed_core_labels(&db).await.unwrap();
        LabelService::register(&db, "custom-label", Some("A custom type"), "my-plugin")
            .await
            .unwrap();

        let labels = LabelService::list(&db).await.unwrap();
        assert!(
            labels.iter().any(|l| l.name == "graphable"),
            "core label should be present"
        );
        assert!(
            labels.iter().any(|l| l.name == "custom-label"),
            "plugin label should be present"
        );
    }
}
