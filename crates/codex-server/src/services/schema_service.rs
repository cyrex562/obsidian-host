use crate::db::Database;
use crate::error::AppResult;
use crate::models::plugin::Plugin;
use crate::models::schema::{
    EntityTypeSchema, EntityTypeToml, PluginLabelDeclaration, RelationTypeSchema, RelationTypeToml,
};
use crate::services::LabelService;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

// ──────────────────────────────────────────────────────────────────────────────
// Registries
// ──────────────────────────────────────────────────────────────────────────────

/// In-memory registry of all entity types across loaded plugins.
/// Keyed by `"<plugin_id>/<type_id>"` (e.g. `"com.example.worldbuilding/character"`).
#[derive(Clone, Default)]
pub struct EntityTypeRegistry {
    inner: Arc<RwLock<HashMap<String, EntityTypeSchema>>>,
}

impl EntityTypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, schema: EntityTypeSchema) {
        let key = format!("{}/{}", schema.plugin_id, schema.id);
        self.inner.write().await.insert(key, schema);
    }

    pub async fn remove_plugin(&self, plugin_id: &str) {
        let prefix = format!("{plugin_id}/");
        self.inner
            .write()
            .await
            .retain(|k, _| !k.starts_with(&prefix));
    }

    pub async fn all(&self) -> Vec<EntityTypeSchema> {
        self.inner.read().await.values().cloned().collect()
    }

    pub async fn get(&self, plugin_id: &str, type_id: &str) -> Option<EntityTypeSchema> {
        let key = format!("{plugin_id}/{type_id}");
        self.inner.read().await.get(&key).cloned()
    }

    pub async fn get_by_id(&self, type_id: &str) -> Option<EntityTypeSchema> {
        self.inner
            .read()
            .await
            .values()
            .find(|s| s.id == type_id)
            .cloned()
    }

    /// Returns all entity types that declare a specific label.
    pub async fn find_types_by_label(&self, label: &str) -> Vec<EntityTypeSchema> {
        self.inner
            .read()
            .await
            .values()
            .filter(|s| s.labels.iter().any(|l| l == label))
            .cloned()
            .collect()
    }
}

/// In-memory registry of all relation types across loaded plugins.
#[derive(Clone, Default)]
pub struct RelationTypeRegistry {
    inner: Arc<RwLock<HashMap<String, RelationTypeSchema>>>,
}

impl RelationTypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, schema: RelationTypeSchema) {
        let key = format!("{}/{}", schema.plugin_id, schema.id);
        self.inner.write().await.insert(key, schema);
    }

    pub async fn remove_plugin(&self, plugin_id: &str) {
        let prefix = format!("{plugin_id}/");
        self.inner
            .write()
            .await
            .retain(|k, _| !k.starts_with(&prefix));
    }

    pub async fn all(&self) -> Vec<RelationTypeSchema> {
        self.inner.read().await.values().cloned().collect()
    }

    /// Find a relation type by its canonical name across all plugins.
    pub async fn find_by_name(&self, name: &str) -> Option<RelationTypeSchema> {
        self.inner
            .read()
            .await
            .values()
            .find(|s| s.name == name)
            .cloned()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Schema loading
// ──────────────────────────────────────────────────────────────────────────────

pub struct SchemaService;

impl SchemaService {
    /// Load all schemas from a slice of enabled plugins. Registers labels,
    /// entity types, and relation types. Called at startup and on plugin toggle.
    pub async fn load_plugin_schemas(
        db: &Database,
        plugins: &[Plugin],
        entity_registry: &EntityTypeRegistry,
        relation_registry: &RelationTypeRegistry,
    ) -> AppResult<()> {
        for plugin in plugins {
            if !plugin.enabled {
                continue;
            }
            Self::load_one(db, plugin, entity_registry, relation_registry).await;
        }
        Ok(())
    }

    async fn load_one(
        db: &Database,
        plugin: &Plugin,
        entity_registry: &EntityTypeRegistry,
        relation_registry: &RelationTypeRegistry,
    ) {
        let plugin_id = &plugin.manifest.id;
        let plugin_dir = Path::new(&plugin.path);

        // Register declared labels
        for decl in &plugin.manifest.labels {
            register_label(db, plugin_id, decl).await;
        }

        // Load entity types
        for rel_path in &plugin.manifest.entity_types {
            let abs = plugin_dir.join(rel_path);
            match load_entity_type_toml(&abs, plugin_id) {
                Ok(schema) => {
                    // Register labels declared in the entity type
                    for label in &schema.labels {
                        let decl = PluginLabelDeclaration {
                            name: label.clone(),
                            description: None,
                        };
                        register_label(db, plugin_id, &decl).await;
                    }
                    info!(
                        "Registered entity type {}/{} from {}",
                        plugin_id, schema.id, rel_path
                    );
                    entity_registry.register(schema).await;
                }
                Err(e) => {
                    warn!("Failed to load entity type {rel_path} for plugin {plugin_id}: {e}");
                }
            }
        }

        // Load relation types
        for rel_path in &plugin.manifest.relation_types {
            let abs = plugin_dir.join(rel_path);
            match load_relation_type_toml(&abs, plugin_id) {
                Ok(schema) => {
                    info!(
                        "Registered relation type {}/{} from {}",
                        plugin_id, schema.id, rel_path
                    );
                    relation_registry.register(schema).await;
                }
                Err(e) => {
                    warn!("Failed to load relation type {rel_path} for plugin {plugin_id}: {e}");
                }
            }
        }
    }

    /// Unload all schemas for a plugin (called when plugin is disabled).
    pub async fn unload_plugin_schemas(
        plugin_id: &str,
        entity_registry: &EntityTypeRegistry,
        relation_registry: &RelationTypeRegistry,
    ) {
        entity_registry.remove_plugin(plugin_id).await;
        relation_registry.remove_plugin(plugin_id).await;
        info!("Unloaded schemas for plugin {plugin_id}");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TOML file loaders
// ──────────────────────────────────────────────────────────────────────────────

fn load_entity_type_toml(path: &Path, plugin_id: &str) -> Result<EntityTypeSchema, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

    let parsed: EntityTypeToml = toml::from_str(&content)
        .map_err(|e| format!("TOML parse error in {}: {e}", path.display()))?;

    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let body = parsed.entity_type;
    Ok(EntityTypeSchema {
        id,
        plugin_id: plugin_id.to_string(),
        name: body.name,
        icon: body.icon,
        color: body.color,
        template: body.template,
        labels: body.labels,
        display_field: body.display_field,
        show_on_create: body.show_on_create,
        fields: body.fields,
    })
}

fn load_relation_type_toml(path: &Path, plugin_id: &str) -> Result<RelationTypeSchema, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

    let parsed: RelationTypeToml = toml::from_str(&content)
        .map_err(|e| format!("TOML parse error in {}: {e}", path.display()))?;

    let id = parsed.relation_type.name.clone();
    let body = parsed.relation_type;

    Ok(RelationTypeSchema {
        id,
        plugin_id: plugin_id.to_string(),
        name: body.name,
        label: body.label,
        from_label: body.from_label,
        to_label: body.to_label,
        directed: body.directed,
        inverse_label: body.inverse_label,
        color: body.color,
        metadata_fields: body.metadata_fields,
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

async fn register_label(db: &Database, plugin_id: &str, decl: &PluginLabelDeclaration) {
    match LabelService::register(db, &decl.name, decl.description.as_deref(), plugin_id).await {
        Ok(_) => {}
        Err(e) => {
            warn!(
                "Failed to register label '{}' for plugin {plugin_id}: {e}",
                decl.name
            );
        }
    }
}

#[cfg(test)]
pub mod tests_helpers {
    use super::*;
    pub fn load_entity_type_toml_pub(
        path: &std::path::Path,
        plugin_id: &str,
    ) -> Result<crate::models::schema::EntityTypeSchema, String> {
        super::load_entity_type_toml(path, plugin_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::schema::{EntityTypeToml, FieldSchema, RelationTypeToml};
    use tempfile::TempDir;

    // ── EntityTypeRegistry tests ──────────────────────────────────────────

    #[tokio::test]
    async fn test_registry_register_and_get() {
        let registry = EntityTypeRegistry::new();
        let schema = EntityTypeSchema {
            id: "character".into(),
            plugin_id: "worldbuilding".into(),
            name: "Character".into(),
            icon: None,
            color: None,
            template: None,
            labels: vec!["graphable".into()],
            display_field: Some("full_name".into()),
            show_on_create: vec!["full_name".into()],
            fields: vec![],
        };
        registry.register(schema.clone()).await;

        let found = registry.get("worldbuilding", "character").await;
        assert!(found.is_some());
        let s = found.unwrap();
        assert_eq!(s.name, "Character");
        assert_eq!(s.display_field.as_deref(), Some("full_name"));
    }

    #[tokio::test]
    async fn test_registry_get_missing_returns_none() {
        let registry = EntityTypeRegistry::new();
        let found = registry.get("nonexistent-plugin", "ghost-type").await;
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_registry_all_returns_all_registered() {
        let registry = EntityTypeRegistry::new();
        for id in ["character", "location", "faction"] {
            registry
                .register(EntityTypeSchema {
                    id: id.into(),
                    plugin_id: "wb".into(),
                    name: id.into(),
                    icon: None,
                    color: None,
                    template: None,
                    labels: vec![],
                    display_field: None,
                    show_on_create: vec![],
                    fields: vec![],
                })
                .await;
        }
        let all = registry.all().await;
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_registry_remove_plugin() {
        let registry = EntityTypeRegistry::new();
        registry
            .register(EntityTypeSchema {
                id: "character".into(),
                plugin_id: "wb".into(),
                name: "Character".into(),
                icon: None,
                color: None,
                template: None,
                labels: vec![],
                display_field: None,
                show_on_create: vec![],
                fields: vec![],
            })
            .await;
        registry
            .register(EntityTypeSchema {
                id: "item".into(),
                plugin_id: "other-plugin".into(),
                name: "Item".into(),
                icon: None,
                color: None,
                template: None,
                labels: vec![],
                display_field: None,
                show_on_create: vec![],
                fields: vec![],
            })
            .await;

        registry.remove_plugin("wb").await;

        assert!(registry.get("wb", "character").await.is_none());
        // other-plugin entry should still be present
        assert!(registry.get("other-plugin", "item").await.is_some());
    }

    // ── TOML parsing tests ────────────────────────────────────────────────

    #[test]
    fn test_entity_type_toml_minimal_parse() {
        let toml_str = r#"
[entity_type]
name = "Character"
"#;
        let parsed: EntityTypeToml = toml::from_str(toml_str).expect("should parse");
        assert_eq!(parsed.entity_type.name, "Character");
        assert!(parsed.entity_type.fields.is_empty());
        assert!(parsed.entity_type.display_field.is_none());
        assert!(parsed.entity_type.show_on_create.is_empty());
    }

    #[test]
    fn test_entity_type_toml_full_parse() {
        let toml_str = r##"
[entity_type]
name = "Character"
icon = "person"
color = "#4A90D9"
display_field = "full_name"
show_on_create = ["full_name", "status"]
labels = ["graphable", "person"]

[[entity_type.fields]]
key = "full_name"
label = "Full Name"
type = "string"
required = true

[[entity_type.fields]]
key = "status"
label = "Status"
type = "enum"
values = ["Active", "Deceased"]
default = "Active"
"##;
        let parsed: EntityTypeToml = toml::from_str(toml_str).expect("should parse");
        let body = &parsed.entity_type;
        assert_eq!(body.name, "Character");
        assert_eq!(body.color.as_deref(), Some("#4A90D9"));
        assert_eq!(body.display_field.as_deref(), Some("full_name"));
        assert_eq!(body.show_on_create, vec!["full_name", "status"]);
        assert_eq!(body.labels, vec!["graphable", "person"]);
        assert_eq!(body.fields.len(), 2);
        assert_eq!(body.fields[0].key, "full_name");
        assert!(body.fields[0].required);
    }

    #[test]
    fn test_load_entity_type_toml_from_real_file() {
        let plugin_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("plugins/worldbuilding/entity_types/character.toml");

        if !plugin_dir.exists() {
            // CI might not have the plugins dir — skip gracefully
            return;
        }

        let schema = super::tests_helpers::load_entity_type_toml_pub(&plugin_dir, "worldbuilding")
            .expect("should load character.toml");

        assert_eq!(schema.id, "character");
        assert_eq!(schema.plugin_id, "worldbuilding");
        assert_eq!(schema.name, "Character");
        assert_eq!(schema.display_field.as_deref(), Some("full_name"));
        assert!(schema.show_on_create.contains(&"full_name".to_string()));
        assert!(!schema.fields.is_empty());
    }

    #[test]
    fn test_relation_type_toml_parse() {
        let toml_str = r#"
[relation_type]
name = "member_of"
label = "Member Of"
from_label = "person"
to_label = "organization"
directed = true
"#;
        let parsed: RelationTypeToml = toml::from_str(toml_str).expect("should parse");
        assert_eq!(parsed.relation_type.name, "member_of");
        assert_eq!(parsed.relation_type.label, "Member Of");
        assert!(parsed.relation_type.directed);
    }

    #[test]
    fn test_load_entity_type_toml_from_temp_file() {
        let temp = TempDir::new().unwrap();
        let toml_content = r#"
[entity_type]
name = "TestType"
display_field = "title"
show_on_create = ["title"]

[[entity_type.fields]]
key = "title"
label = "Title"
type = "string"
"#;
        let path = temp.path().join("testtype.toml");
        std::fs::write(&path, toml_content).unwrap();
        let schema = super::tests_helpers::load_entity_type_toml_pub(&path, "test-plugin")
            .expect("should load temp TOML");
        assert_eq!(schema.id, "testtype");
        assert_eq!(schema.name, "TestType");
        assert_eq!(schema.fields.len(), 1);
    }
}
