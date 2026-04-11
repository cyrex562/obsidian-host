use crate::error::{AppError, AppResult};
use crate::services::schema_service::EntityTypeRegistry;
use std::path::Path;

pub struct TemplateService;

impl TemplateService {
    /// Return the template markdown content for an entity type.
    /// The template file path is resolved relative to the plugin directory
    /// (stored in the entity type schema), falling back to a minimal
    /// generated template when the file is absent.
    pub async fn get_template(
        registry: &EntityTypeRegistry,
        entity_type_id: &str,
        plugins_dir: &Path,
    ) -> AppResult<String> {
        let schema = registry.get_by_id(entity_type_id).await.ok_or_else(|| {
            AppError::NotFound(format!("Entity type not found: {entity_type_id}"))
        })?;

        // If the plugin declared a template file, try to read it
        if let Some(ref template_rel) = schema.template {
            // The plugin dir lives at <plugins_dir>/<plugin_id_last_segment>
            // We stored the full plugin directory path in the registry implicitly
            // via SchemaService — but PluginService stores plugin.path.
            // For now we derive the plugin dir from plugins_dir + plugin_id basename.
            let plugin_basename = schema
                .plugin_id
                .rsplit('.')
                .next()
                .unwrap_or(&schema.plugin_id);
            let template_abs = plugins_dir.join(plugin_basename).join(template_rel);
            if let Ok(content) = tokio::fs::read_to_string(&template_abs).await {
                return Ok(content);
            }
        }

        // Fallback: generate a minimal template from the schema
        Ok(generate_minimal_template(&schema))
    }
}

fn generate_minimal_template(schema: &crate::models::schema::EntityTypeSchema) -> String {
    use std::fmt::Write;
    let mut fm = String::new();
    writeln!(fm, "---").unwrap();
    writeln!(fm, "codex_type: {}", schema.name).unwrap();
    writeln!(fm, "codex_plugin: {}", schema.plugin_id).unwrap();
    if !schema.labels.is_empty() {
        writeln!(fm, "codex_labels:").unwrap();
        for label in &schema.labels {
            writeln!(fm, "  - {label}").unwrap();
        }
    }
    for field in &schema.fields {
        let default_val = field
            .default
            .as_ref()
            .map(|v| {
                if let Some(s) = v.as_str() {
                    format!(" {s}")
                } else {
                    format!(" {v}")
                }
            })
            .unwrap_or_else(|| " \"\"".to_string());
        writeln!(fm, "{}:{}", field.key, default_val).unwrap();
    }
    writeln!(fm, "---").unwrap();
    writeln!(fm).unwrap();
    writeln!(fm, "<!-- codex:prose:begin -->").unwrap();
    writeln!(fm).unwrap();
    writeln!(fm, "<!-- codex:prose:end -->").unwrap();
    fm
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::schema::{EntityTypeSchema, FieldSchema, FieldType};

    fn make_schema(
        id: &str,
        plugin_id: &str,
        labels: Vec<String>,
        fields: Vec<FieldSchema>,
    ) -> EntityTypeSchema {
        EntityTypeSchema {
            id: id.into(),
            plugin_id: plugin_id.into(),
            name: id.into(),
            icon: None,
            color: None,
            template: None,
            labels,
            display_field: None,
            show_on_create: vec![],
            fields,
        }
    }

    fn make_field(
        key: &str,
        field_type: FieldType,
        default: Option<serde_json::Value>,
    ) -> FieldSchema {
        FieldSchema {
            key: key.into(),
            label: key.into(),
            field_type,
            required: false,
            item_type: None,
            values: vec![],
            default,
            target_label: None,
            relation: None,
            description: None,
        }
    }

    // ── generate_minimal_template ─────────────────────────────────────────

    #[test]
    fn test_minimal_template_contains_frontmatter_delimiters() {
        let schema = make_schema("character", "worldbuilding", vec![], vec![]);
        let tmpl = generate_minimal_template(&schema);
        assert!(
            tmpl.starts_with("---\n"),
            "should start with front-matter opener"
        );
        assert!(
            tmpl.contains("\n---\n"),
            "should contain front-matter closer"
        );
    }

    #[test]
    fn test_minimal_template_contains_codex_type() {
        let schema = make_schema("character", "worldbuilding", vec![], vec![]);
        let tmpl = generate_minimal_template(&schema);
        assert!(
            tmpl.contains("codex_type: character"),
            "should set codex_type"
        );
    }

    #[test]
    fn test_minimal_template_contains_plugin_id() {
        let schema = make_schema("location", "com.codex.worldbuilding", vec![], vec![]);
        let tmpl = generate_minimal_template(&schema);
        assert!(tmpl.contains("codex_plugin: com.codex.worldbuilding"));
    }

    #[test]
    fn test_minimal_template_labels_emitted_when_present() {
        let schema = make_schema(
            "character",
            "wb",
            vec!["graphable".into(), "person".into()],
            vec![],
        );
        let tmpl = generate_minimal_template(&schema);
        assert!(
            tmpl.contains("codex_labels:"),
            "should emit codex_labels section"
        );
        assert!(tmpl.contains("  - graphable"));
        assert!(tmpl.contains("  - person"));
    }

    #[test]
    fn test_minimal_template_no_labels_section_when_empty() {
        let schema = make_schema("event", "wb", vec![], vec![]);
        let tmpl = generate_minimal_template(&schema);
        assert!(
            !tmpl.contains("codex_labels:"),
            "no labels block when labels is empty"
        );
    }

    #[test]
    fn test_minimal_template_fields_get_empty_default() {
        let schema = make_schema(
            "character",
            "wb",
            vec![],
            vec![make_field("full_name", FieldType::String, None)],
        );
        let tmpl = generate_minimal_template(&schema);
        assert!(
            tmpl.contains("full_name: \"\""),
            "string field with no default gets empty string"
        );
    }

    #[test]
    fn test_minimal_template_field_with_string_default() {
        let schema = make_schema(
            "character",
            "wb",
            vec![],
            vec![make_field(
                "status",
                FieldType::Enum,
                Some(serde_json::json!("Active")),
            )],
        );
        let tmpl = generate_minimal_template(&schema);
        assert!(
            tmpl.contains("status: Active"),
            "enum field with default uses default value"
        );
    }

    #[test]
    fn test_minimal_template_contains_prose_markers() {
        let schema = make_schema("character", "wb", vec![], vec![]);
        let tmpl = generate_minimal_template(&schema);
        assert!(tmpl.contains("<!-- codex:prose:begin -->"));
        assert!(tmpl.contains("<!-- codex:prose:end -->"));
    }

    #[test]
    fn test_minimal_template_multiple_fields() {
        let schema = make_schema(
            "faction",
            "wb",
            vec![],
            vec![
                make_field("name", FieldType::String, None),
                make_field(
                    "alignment",
                    FieldType::Enum,
                    Some(serde_json::json!("Neutral")),
                ),
                make_field("founded", FieldType::Date, None),
            ],
        );
        let tmpl = generate_minimal_template(&schema);
        assert!(tmpl.contains("name: \"\""));
        assert!(tmpl.contains("alignment: Neutral"));
        assert!(tmpl.contains("founded: \"\""));
    }

    // ── TemplateService::get_template — registry miss ────────────────────

    #[tokio::test]
    async fn test_get_template_returns_error_for_unknown_type() {
        let registry = crate::services::schema_service::EntityTypeRegistry::new();
        let result = TemplateService::get_template(&registry, "ghost_type", "").await;
        assert!(result.is_err(), "missing entity type should return error");
    }

    #[tokio::test]
    async fn test_get_template_returns_fallback_when_no_file() {
        let registry = crate::services::schema_service::EntityTypeRegistry::new();
        let schema = EntityTypeSchema {
            id: "character".into(),
            plugin_id: "worldbuilding".into(),
            name: "character".into(),
            icon: None,
            color: None,
            template: None, // No template file
            labels: vec!["graphable".into()],
            display_field: Some("full_name".into()),
            show_on_create: vec![],
            fields: vec![make_field("full_name", FieldType::String, None)],
        };
        registry.register(schema).await;

        let content = TemplateService::get_template(&registry, "character", "/nonexistent/dir")
            .await
            .expect("should return generated fallback");
        assert!(
            content.contains("codex_type:"),
            "fallback should include type field"
        );
        assert!(
            content.contains("full_name: \"\""),
            "fallback should include field"
        );
    }

    #[tokio::test]
    async fn test_get_template_reads_real_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let plugin_dir = temp.path().join("worldbuilding");
        let tmpl_dir = plugin_dir.join("templates");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        let tmpl_content = "---\ncodex_type: character\n---\n# New Character\n";
        std::fs::write(tmpl_dir.join("character.md"), tmpl_content).unwrap();

        let registry = crate::services::schema_service::EntityTypeRegistry::new();
        let schema = EntityTypeSchema {
            id: "character".into(),
            plugin_id: "com.codex.worldbuilding".into(),
            name: "character".into(),
            icon: None,
            color: None,
            template: Some("templates/character.md".into()),
            labels: vec![],
            display_field: None,
            show_on_create: vec![],
            fields: vec![],
        };
        registry.register(schema).await;

        let content =
            TemplateService::get_template(&registry, "character", temp.path().to_str().unwrap())
                .await
                .expect("should read template from disk");
        assert_eq!(content, tmpl_content);
    }
}
