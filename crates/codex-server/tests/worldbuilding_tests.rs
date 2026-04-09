/// Integration tests for the worldbuilding feature set:
/// entities, relations, graph, label list, filters, and reindex edge cases.
use actix_web::{test, web, App};
use codex::db::Database;
use codex::models::EntityTypeSchema;
use codex::routes::{entities, AppState};
use codex::services::{
    EntityService, EntityTypeRegistry, LabelService, MarkdownParser, ReindexService,
    RelationService, RelationTypeRegistry, SearchIndex,
};
use codex::watcher::FileWatcher;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{broadcast, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

async fn setup(temp_dir: &TempDir) -> (web::Data<AppState>, String) {
    let db_path = temp_dir.path().join("wb-integration.db");
    let db_url = format!("sqlite://{}", db_path.display());
    let db = Database::new(&db_url).await.unwrap();

    let search_index = SearchIndex::new();
    let (watcher, _) = FileWatcher::new().unwrap();
    let watcher = Arc::new(Mutex::new(watcher));
    let (event_tx, _) = broadcast::channel(100);

    let state = web::Data::new(AppState {
        db: db.clone(),
        search_index,
        watcher,
        event_broadcaster: event_tx,
        ws_broadcaster: tokio::sync::broadcast::channel::<codex::models::WsMessage>(16).0,
        change_log_retention_days: 7,
        ml_undo_store: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shutdown_tx: broadcast::channel::<()>(1).0,
        document_parser: Arc::new(MarkdownParser),
        entity_type_registry: EntityTypeRegistry::new(),
        relation_type_registry: RelationTypeRegistry::new(),
        plugins_dir: String::new(),
    });

    let vault_dir = temp_dir.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();
    let vault = db
        .create_vault("WB Vault".into(), vault_dir.to_string_lossy().into())
        .await
        .unwrap();

    (state, vault.id)
}

/// Write an entity markdown file with the given fields and return its path.
fn write_entity(
    vault_dir: &std::path::Path,
    filename: &str,
    entity_type: &str,
    extra_fields: &[(&str, &str)],
) {
    let fields: String = extra_fields
        .iter()
        .map(|(k, v)| format!("{k}: {v}\n"))
        .collect();
    let content = format!(
        "---\ncodex_type: {entity_type}\ncodex_plugin: worldbuilding\ncodex_labels:\n  - graphable\n{fields}---\n# Content\n"
    );
    std::fs::write(vault_dir.join(filename), content).unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// Entity list filter tests
// ─────────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_entity_list_filter_by_entity_type() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    write_entity(&vault_dir, "alice.md", "character", &[("full_name", "Alice")]);
    write_entity(&vault_dir, "city.md", "location", &[("full_name", "The City")]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/entities?entity_type=character"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let list = body["entities"].as_array().unwrap();
    assert_eq!(list.len(), 1, "filter by entity_type=character should return 1");
    assert_eq!(list[0]["entity_type"].as_str(), Some("character"));
}

#[actix_web::test]
async fn test_entity_list_filter_by_label() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    // Both files will get the "graphable" label from the frontmatter
    write_entity(&vault_dir, "faction.md", "faction", &[]);
    write_entity(&vault_dir, "location.md", "location", &[]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/entities?label=graphable"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let list = body["entities"].as_array().unwrap();
    assert_eq!(list.len(), 2, "both entities carry the 'graphable' label");
}

#[actix_web::test]
async fn test_entity_list_filter_by_name_query() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    write_entity(&vault_dir, "aria.md", "character", &[("full_name", "Aria Stormwind")]);
    write_entity(&vault_dir, "lyra.md", "character", &[("full_name", "Lyra Brightwood")]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    // Query "stormwind" — should only match Aria
    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/entities?q=stormwind"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let list = body["entities"].as_array().unwrap();
    assert_eq!(list.len(), 1, "q=stormwind should match Aria only");
}

#[actix_web::test]
async fn test_entity_list_filter_by_plugin() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    // Standard worldbuilding entity
    write_entity(&vault_dir, "hero.md", "character", &[]);
    // A manually crafted entity from a different plugin
    let other_content = "---\ncodex_type: item\ncodex_plugin: inventory-plugin\n---\n";
    std::fs::write(vault_dir.join("sword.md"), other_content).unwrap();

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/entities?plugin=worldbuilding"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let list = body["entities"].as_array().unwrap();
    assert_eq!(list.len(), 1, "plugin filter should only return worldbuilding entities");
    assert_eq!(list[0]["plugin_id"].as_str(), Some("worldbuilding"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Entity-by-ID endpoint
// ─────────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_get_entity_by_id() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    write_entity(&vault_dir, "hero.md", "character", &[("full_name", "The Hero")]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let entities = EntityService::list_all_in_vault(&state.db, &vault_id)
        .await
        .unwrap();
    let entity_id = &entities[0].id;

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/entities/{entity_id}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "get_entity_by_id should return 200");

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["entity_type"].as_str(), Some("character"));
    assert_eq!(body["path"].as_str(), Some("hero.md"));
}

#[actix_web::test]
async fn test_get_entity_by_id_not_found() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/entities/nonexistent-id-00000"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 404);
}

// ─────────────────────────────────────────────────────────────────────────────
// Entity relations endpoint
// ─────────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_entity_relations_endpoint_empty() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    write_entity(&vault_dir, "lone.md", "character", &[]);
    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let entities = EntityService::list_all_in_vault(&state.db, &vault_id)
        .await
        .unwrap();
    let entity_id = &entities[0].id;

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/entities/{entity_id}/relations"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["relations"].is_array());
    assert_eq!(body["relations"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_entity_relations_endpoint_with_linked_entities() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    // Alice belongs to the Order faction (wiki-link reference)
    write_entity(
        &vault_dir,
        "alice.md",
        "character",
        &[("faction", "\"[[Order]]\"")],
    );
    write_entity(&vault_dir, "Order.md", "faction", &[]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let entities = EntityService::list_all_in_vault(&state.db, &vault_id)
        .await
        .unwrap();
    let alice = entities.iter().find(|e| e.path == "alice.md").unwrap();

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/vaults/{vault_id}/entities/{}/relations",
            alice.id
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let relations = body["relations"].as_array().expect("relations array");
    assert!(!relations.is_empty(), "alice should have a relation to Order");

    // Check that forward relation goes from alice → order
    let forward = relations
        .iter()
        .find(|r| r["direction"].as_str() == Some("forward"));
    assert!(forward.is_some(), "should have a forward direction relation");
}

// ─────────────────────────────────────────────────────────────────────────────
// Graph endpoint with relations
// ─────────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_graph_contains_edges_for_wiki_links() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    write_entity(
        &vault_dir,
        "hero.md",
        "character",
        &[("location", "\"[[TowerCity]]\"")],
    );
    write_entity(&vault_dir, "TowerCity.md", "location", &[]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/graph"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let nodes = body["nodes"].as_array().unwrap();
    let edges = body["edges"].as_array().unwrap();

    assert_eq!(nodes.len(), 2, "should have 2 graph nodes");
    assert!(!edges.is_empty(), "should have at least one edge from wiki-link");

    // Edge fields
    let edge = &edges[0];
    assert!(edge["id"].is_string());
    assert!(edge["source"].is_string());
    assert!(edge["target"].is_string());
    assert!(edge["relation_type"].is_string());
}

#[actix_web::test]
async fn test_graph_node_has_expected_fields() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    write_entity(&vault_dir, "hero.md", "character", &[]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/graph"))
        .to_request();
    let body: serde_json::Value =
        test::read_body_json(test::call_service(&app, req).await).await;

    let node = &body["nodes"].as_array().unwrap()[0];
    assert!(node["id"].is_string(), "node.id should be a string");
    assert!(node["path"].is_string(), "node.path should be a string");
    assert!(node["entity_type"].is_string(), "node.entity_type should be a string");
    assert!(node["labels"].is_array(), "node.labels should be an array");
    assert!(node["title"].is_string(), "node.title should be a string");
    // title should be derived from path basename (without extension)
    assert_eq!(node["title"].as_str(), Some("hero"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Label list endpoint
// ─────────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_labels_endpoint_empty_db() {
    let temp = TempDir::new().unwrap();
    let (state, _vault_id) = setup(&temp).await;

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri("/api/plugins/labels")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["labels"].is_array());
}

#[actix_web::test]
async fn test_labels_endpoint_after_seed() {
    let temp = TempDir::new().unwrap();
    let (state, _vault_id) = setup(&temp).await;

    LabelService::seed_core_labels(&state.db).await.unwrap();

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri("/api/plugins/labels")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let labels = body["labels"].as_array().unwrap();
    assert!(
        labels.len() >= 8,
        "should return at least 8 core labels after seed"
    );

    let graphable = labels.iter().find(|l| l["name"].as_str() == Some("graphable"));
    assert!(graphable.is_some(), "should contain 'graphable' label");
    assert_eq!(graphable.unwrap()["source"].as_str(), Some("core"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Schema-aware entity type template endpoint
// ─────────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_entity_type_template_endpoint_with_registered_schema() {
    let temp = TempDir::new().unwrap();
    let (state, _vault_id) = setup(&temp).await;

    // Register a schema with no template file (triggers fallback generation)
    state
        .entity_type_registry
        .register(EntityTypeSchema {
            id: "character".into(),
            plugin_id: "worldbuilding".into(),
            name: "character".into(),
            icon: None,
            color: None,
            template: None,
            labels: vec!["graphable".into()],
            display_field: Some("full_name".into()),
            show_on_create: vec!["full_name".into()],
            fields: vec![],
        })
        .await;

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri("/api/plugins/entity-types/character/template")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "should return 200 for registered type");

    let body: serde_json::Value = test::read_body_json(resp).await;
    let content = body["content"].as_str().expect("content field should be a string");
    assert!(content.contains("codex_type:"), "template should contain codex_type");
}

#[actix_web::test]
async fn test_entity_type_template_endpoint_missing_type_returns_404() {
    let temp = TempDir::new().unwrap();
    let (state, _vault_id) = setup(&temp).await;

    let app =
        test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri("/api/plugins/entity-types/nonexistent-type/template")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 404);
}

// ─────────────────────────────────────────────────────────────────────────────
// Reindex edge cases
// ─────────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_reindex_multiple_entity_types() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    write_entity(&vault_dir, "aria.md", "character", &[]);
    write_entity(&vault_dir, "city.md", "location", &[]);
    write_entity(&vault_dir, "guild.md", "faction", &[]);
    write_entity(&vault_dir, "battle.md", "event", &[]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let entities = EntityService::list_all_in_vault(&state.db, &vault_id)
        .await
        .unwrap();
    assert_eq!(entities.len(), 4, "all 4 entity types should be indexed");

    let types: std::collections::HashSet<String> =
        entities.iter().map(|e| e.entity_type.clone()).collect();
    for expected_type in ["character", "location", "faction", "event"] {
        assert!(types.contains(expected_type), "missing type: {expected_type}");
    }
}

#[actix_web::test]
async fn test_reindex_in_nested_directory() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    let chars_dir = vault_dir.join("Characters");
    std::fs::create_dir_all(&chars_dir).unwrap();
    write_entity(&chars_dir, "hero.md", "character", &[]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let entities = EntityService::list_all_in_vault(&state.db, &vault_id)
        .await
        .unwrap();
    assert_eq!(entities.len(), 1);
    // Path should be relative and use forward slash
    assert_eq!(entities[0].path, "Characters/hero.md");
}

#[actix_web::test]
async fn test_reindex_relation_created_for_cross_linked_entities() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    write_entity(
        &vault_dir,
        "aria.md",
        "character",
        &[("mentor", "\"[[Elder]]\"")],
    );
    write_entity(&vault_dir, "Elder.md", "character", &[]);

    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .unwrap();

    let entities = EntityService::list_all_in_vault(&state.db, &vault_id)
        .await
        .unwrap();
    let aria = entities.iter().find(|e| e.path == "aria.md").unwrap();

    let relations = RelationService::get_for_entity(&state.db, &aria.id)
        .await
        .unwrap();
    assert!(!relations.is_empty(), "aria should have a relation to Elder");
}

#[actix_web::test]
async fn test_reindex_unresolved_ref_does_not_fail() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;
    let vault_dir = temp.path().join("vault");

    // Reference to an entity that doesn't exist yet
    write_entity(
        &vault_dir,
        "hero.md",
        "character",
        &[("guild", "\"[[MissingGuild]]\"")],
    );

    // This should not error — unresolved refs are silently skipped
    let result = ReindexService::reindex_vault(
        &state.db,
        &vault_id,
        vault_dir.to_str().unwrap(),
    )
    .await;
    assert!(result.is_ok(), "unresolved wiki-link should not fail reindex");

    // Entity is still indexed, just has no relations
    let entities = EntityService::list_all_in_vault(&state.db, &vault_id)
        .await
        .unwrap();
    assert_eq!(entities.len(), 1);
    let relations = RelationService::get_for_entity(&state.db, &entities[0].id)
        .await
        .unwrap();
    assert!(relations.is_empty(), "no relations for unresolved ref");
}
