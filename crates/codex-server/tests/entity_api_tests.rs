use actix_web::{test, web, App};
use codex::db::Database;
use codex::routes::{entities, AppState};
use codex::services::{
    EntityTypeRegistry, MarkdownParser, ReindexService, RelationTypeRegistry, SearchIndex,
};
use codex::watcher::FileWatcher;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{broadcast, Mutex};

async fn setup(temp_dir: &TempDir) -> (web::Data<AppState>, String) {
    let db_path = temp_dir.path().join("entity-test.db");
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
        .create_vault("Test Vault".into(), vault_dir.to_string_lossy().into())
        .await
        .unwrap();

    (state, vault.id)
}

// ── Entity schema endpoints ────────────────────────────────────────────────

#[actix_web::test]
async fn test_list_entity_types_empty_registry() {
    let temp = TempDir::new().unwrap();
    let (state, _vault_id) = setup(&temp).await;

    let app = test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri("/api/plugins/entity-types")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["entity_types"].is_array());
    assert_eq!(body["entity_types"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_list_entity_types_with_registered_type() {
    let temp = TempDir::new().unwrap();
    let (state, _vault_id) = setup(&temp).await;

    // Pre-register a type
    state
        .entity_type_registry
        .register(codex::models::EntityTypeSchema {
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
        })
        .await;

    let app = test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri("/api/plugins/entity-types")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let types = body["entity_types"]
        .as_array()
        .expect("should have entity_types array");
    assert_eq!(types.len(), 1);
    assert_eq!(types[0]["id"].as_str(), Some("character"));
}

#[actix_web::test]
async fn test_list_relation_types_empty_registry() {
    let temp = TempDir::new().unwrap();
    let (state, _vault_id) = setup(&temp).await;

    let app = test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri("/api/plugins/relation-types")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["relation_types"].is_array());
    assert_eq!(body["relation_types"].as_array().unwrap().len(), 0);
}

// ── Entity CRUD endpoints ─────────────────────────────────────────────────

#[actix_web::test]
async fn test_list_entities_empty_vault() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;

    let app = test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/entities"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["entities"].is_array());
    assert_eq!(body["entities"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_reindex_vault() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;

    // Write an entity markdown file
    let vault_dir = temp.path().join("vault");
    let content = "---\ncodex_type: character\ncodex_plugin: worldbuilding\ncodex_labels:\n- graphable\nfull_name: Alice Smith\n---\n# Alice Smith\n";
    std::fs::write(vault_dir.join("alice.md"), content).unwrap();

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(entities::configure),
    )
    .await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{vault_id}/reindex"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    // 202 Accepted — reindex is async
    assert_eq!(resp.status().as_u16(), 202);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["vault_id"].as_str(), Some(vault_id.as_str()));

    // Run reindex directly to verify it works synchronously
    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .expect("reindex should succeed");

    // Verify entity is now in the DB
    let list_req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/entities"))
        .to_request();
    let list_resp = test::call_service(&app, list_req).await;
    let entities_body: serde_json::Value = test::read_body_json(list_resp).await;
    let entities = entities_body["entities"]
        .as_array()
        .expect("entities array");
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0]["entity_type"].as_str(), Some("character"));
}

#[actix_web::test]
async fn test_entity_by_path_not_found() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;

    let app = test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/vaults/{vault_id}/entity-by-path?path=nonexistent.md"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    // 200 with null or 404 — either is acceptable; we verify entity is null
    let status = resp.status();
    if status.is_success() {
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["entity"].is_null());
    } else {
        assert_eq!(status.as_u16(), 404);
    }
}

#[actix_web::test]
async fn test_entity_by_path_found_after_reindex() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;

    let vault_dir = temp.path().join("vault");
    let content = "---\ncodex_type: location\ncodex_plugin: worldbuilding\ncodex_labels:\n- graphable\nname: Castle Keep\n---\n# Castle Keep\n";
    std::fs::write(vault_dir.join("castle.md"), content).unwrap();

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(entities::configure),
    )
    .await;

    // Reindex synchronously so entities are in DB before querying
    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .expect("reindex should succeed");

    // Now query by path
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/vaults/{vault_id}/entity-by-path?path=castle.md"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(!body["entity"].is_null());
    assert_eq!(body["entity"]["entity_type"].as_str(), Some("location"));
    assert_eq!(body["entity"]["path"].as_str(), Some("castle.md"));
    assert!(body["relations"].is_array());
}

// ── Graph endpoint ─────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_get_graph_empty_vault() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;

    let app = test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/graph"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["nodes"].is_array());
    assert!(body["edges"].is_array());
    assert_eq!(body["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(body["edges"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_get_graph_with_indexed_entities() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;

    let vault_dir = temp.path().join("vault");
    for (filename, entity_type, name) in [
        ("hero.md", "character", "The Hero"),
        ("city.md", "location", "The City"),
    ] {
        let content = format!("---\ncodex_type: {entity_type}\ncodex_plugin: worldbuilding\ncodex_labels:\n- graphable\nname: {name}\n---\n# {name}\n");
        std::fs::write(vault_dir.join(filename), content).unwrap();
    }

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(entities::configure),
    )
    .await;

    // Reindex synchronously
    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .expect("reindex should succeed");

    // Get graph
    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{vault_id}/graph"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let nodes = body["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);

    // Each node should have id, path, entity_type, labels, title
    for node in nodes {
        assert!(node["id"].is_string());
        assert!(node["path"].is_string());
        assert!(node["entity_type"].is_string());
        assert!(node["labels"].is_array());
    }
}

// ── Entity template endpoint ───────────────────────────────────────────────

#[actix_web::test]
async fn test_entity_template_no_schema_returns_error_or_default() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;

    let app = test::init_service(App::new().app_data(state).configure(entities::configure)).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/vaults/{vault_id}/entity-template?type=character&plugin=worldbuilding"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Without a registered schema or template file, expect 404 or an empty template
    let status = resp.status().as_u16();
    assert!(
        status == 200 || status == 404,
        "Expected 200 or 404, got {status}"
    );
}

// ── Entity index stats (direct DB verification) ────────────────────────────

#[actix_web::test]
async fn test_entity_count_in_db_after_reindex() {
    let temp = TempDir::new().unwrap();
    let (state, vault_id) = setup(&temp).await;

    let vault_dir = temp.path().join("vault");
    let content = "---\ncodex_type: character\ncodex_plugin: worldbuilding\n---\n# Hero\n";
    std::fs::write(vault_dir.join("hero.md"), content).unwrap();

    // Reindex synchronously
    ReindexService::reindex_vault(&state.db, &vault_id, vault_dir.to_str().unwrap())
        .await
        .expect("reindex should succeed");

    // Verify entity count directly via DB query (bypasses admin auth)
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entities WHERE vault_id = ?")
        .bind(&vault_id)
        .fetch_one(state.db.pool())
        .await
        .expect("count query should succeed");

    assert_eq!(count.0, 1, "should have 1 indexed entity");
}
