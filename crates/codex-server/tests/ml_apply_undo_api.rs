use actix_web::{test, web, App};
use codex::db::Database;
use codex::routes::{ml, AppState};
use codex::services::{MarkdownParser, SearchIndex};
use codex::watcher::FileWatcher;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{broadcast, Mutex};

#[actix_web::test]
async fn apply_tag_and_undo_restores_file_and_receipt_is_single_use() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("ml-tag-undo.db");
    let db_url = format!("sqlite://{}", db_path.display());
    let db = Database::new(&db_url).await.unwrap();

    let vault_path = temp_dir.path().join("vault");
    std::fs::create_dir_all(&vault_path).unwrap();

    let note_rel = "note.md";
    let note_abs = vault_path.join(note_rel);
    std::fs::write(&note_abs, "# Demo\n\nhello\n").unwrap();

    let vault = db
        .create_vault(
            "ML Test Vault".to_string(),
            vault_path.to_string_lossy().to_string(),
        )
        .await
        .unwrap();

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
        ml_undo_store: Arc::new(Mutex::new(HashMap::new())),
    shutdown_tx: tokio::sync::broadcast::channel::<()>(1).0,
        document_parser: Arc::new(MarkdownParser),
        entity_type_registry: codex::services::EntityTypeRegistry::new(),
        relation_type_registry: codex::services::RelationTypeRegistry::new(),
        plugins_dir: String::new(),
    });

    let app = test::init_service(App::new().app_data(state.clone()).configure(ml::configure)).await;

    let apply_req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/ml/apply-suggestion", vault.id))
        .set_json(json!({
            "file_path": note_rel,
            "dry_run": false,
            "suggestion": {
                "id": "s-tag-1",
                "kind": "tag",
                "confidence": 0.95,
                "rationale": "Looks like project work",
                "tag": "project"
            }
        }))
        .to_request();

    let apply_resp = test::call_service(&app, apply_req).await;
    assert!(apply_resp.status().is_success());
    let apply_body: serde_json::Value = test::read_body_json(apply_resp).await;
    assert_eq!(apply_body["applied"], true);
    let receipt_id = apply_body["receipt_id"].as_str().unwrap().to_string();

    let content_after_apply = std::fs::read_to_string(&note_abs).unwrap();
    assert!(content_after_apply.contains("project"));

    let undo_req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/ml/undo", vault.id))
        .set_json(json!({ "receipt_id": receipt_id }))
        .to_request();

    let undo_resp = test::call_service(&app, undo_req).await;
    assert!(undo_resp.status().is_success());
    let undo_body: serde_json::Value = test::read_body_json(undo_resp).await;
    assert_eq!(undo_body["undone"], true);

    let content_after_undo = std::fs::read_to_string(&note_abs).unwrap();
    assert!(!content_after_undo.contains("project"));

    let undo_again_req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/ml/undo", vault.id))
        .set_json(json!({ "receipt_id": undo_body["receipt_id"] }))
        .to_request();

    let undo_again_resp = test::call_service(&app, undo_again_req).await;
    assert_eq!(undo_again_resp.status().as_u16(), 404);
}

#[actix_web::test]
async fn apply_move_and_undo_restores_original_path() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("ml-move-undo.db");
    let db_url = format!("sqlite://{}", db_path.display());
    let db = Database::new(&db_url).await.unwrap();

    let vault_path = temp_dir.path().join("vault");
    let inbox_dir = vault_path.join("inbox");
    let archive_dir = vault_path.join("archive");
    std::fs::create_dir_all(&inbox_dir).unwrap();
    std::fs::create_dir_all(&archive_dir).unwrap();

    let from_rel = "inbox/task.md";
    let from_abs = vault_path.join(from_rel);
    let to_rel = "archive/task.md";
    let to_abs = vault_path.join(to_rel);
    std::fs::write(&from_abs, "# Task\n\nmove me\n").unwrap();

    let vault = db
        .create_vault(
            "ML Move Vault".to_string(),
            vault_path.to_string_lossy().to_string(),
        )
        .await
        .unwrap();

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
        ml_undo_store: Arc::new(Mutex::new(HashMap::new())),
    shutdown_tx: tokio::sync::broadcast::channel::<()>(1).0,
        document_parser: Arc::new(MarkdownParser),
        entity_type_registry: codex::services::EntityTypeRegistry::new(),
        relation_type_registry: codex::services::RelationTypeRegistry::new(),
        plugins_dir: String::new(),
    });

    let app = test::init_service(App::new().app_data(state.clone()).configure(ml::configure)).await;

    let apply_req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/ml/apply-suggestion", vault.id))
        .set_json(json!({
            "file_path": from_rel,
            "dry_run": false,
            "suggestion": {
                "id": "s-move-1",
                "kind": "move_to_folder",
                "confidence": 0.91,
                "rationale": "Archive completed notes",
                "target_folder": "archive"
            }
        }))
        .to_request();

    let apply_resp = test::call_service(&app, apply_req).await;
    assert!(apply_resp.status().is_success());
    let apply_body: serde_json::Value = test::read_body_json(apply_resp).await;
    assert_eq!(apply_body["updated_file_path"], to_rel);
    let receipt_id = apply_body["receipt_id"].as_str().unwrap().to_string();

    assert!(!from_abs.exists());
    assert!(to_abs.exists());

    let undo_req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/ml/undo", vault.id))
        .set_json(json!({ "receipt_id": receipt_id }))
        .to_request();

    let undo_resp = test::call_service(&app, undo_req).await;
    assert!(undo_resp.status().is_success());

    assert!(from_abs.exists());
    assert!(!to_abs.exists());
}

#[actix_web::test]
async fn undo_receipt_persists_across_app_reinitialization() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("ml-persisted-receipt.db");
    let db_url = format!("sqlite://{}", db_path.display());
    let db = Database::new(&db_url).await.unwrap();

    let vault_path = temp_dir.path().join("vault");
    std::fs::create_dir_all(&vault_path).unwrap();

    let note_rel = "persist.md";
    let note_abs = vault_path.join(note_rel);
    std::fs::write(&note_abs, "# Persist\n\nhello\n").unwrap();

    let vault = db
        .create_vault(
            "ML Persisted Receipt Vault".to_string(),
            vault_path.to_string_lossy().to_string(),
        )
        .await
        .unwrap();

    let receipt_id = {
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
            ml_undo_store: Arc::new(Mutex::new(HashMap::new())),
        shutdown_tx: tokio::sync::broadcast::channel::<()>(1).0,
            document_parser: Arc::new(MarkdownParser),
            entity_type_registry: codex::services::EntityTypeRegistry::new(),
            relation_type_registry: codex::services::RelationTypeRegistry::new(),
            plugins_dir: String::new(),
        });

        let app =
            test::init_service(App::new().app_data(state.clone()).configure(ml::configure)).await;

        let apply_req = test::TestRequest::post()
            .uri(&format!("/api/vaults/{}/ml/apply-suggestion", vault.id))
            .set_json(json!({
                "file_path": note_rel,
                "dry_run": false,
                "suggestion": {
                    "id": "s-tag-persist-1",
                    "kind": "tag",
                    "confidence": 0.9,
                    "rationale": "Persisted receipt check",
                    "tag": "persisted"
                }
            }))
            .to_request();

        let apply_resp = test::call_service(&app, apply_req).await;
        assert!(apply_resp.status().is_success());
        let apply_body: serde_json::Value = test::read_body_json(apply_resp).await;
        apply_body["receipt_id"].as_str().unwrap().to_string()
    };

    let content_after_apply = std::fs::read_to_string(&note_abs).unwrap();
    assert!(content_after_apply.contains("persisted"));

    {
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
            ml_undo_store: Arc::new(Mutex::new(HashMap::new())),
        shutdown_tx: tokio::sync::broadcast::channel::<()>(1).0,
            document_parser: Arc::new(MarkdownParser),
            entity_type_registry: codex::services::EntityTypeRegistry::new(),
            relation_type_registry: codex::services::RelationTypeRegistry::new(),
            plugins_dir: String::new(),
        });

        let app =
            test::init_service(App::new().app_data(state.clone()).configure(ml::configure)).await;

        let undo_req = test::TestRequest::post()
            .uri(&format!("/api/vaults/{}/ml/undo", vault.id))
            .set_json(json!({ "receipt_id": receipt_id }))
            .to_request();

        let undo_resp = test::call_service(&app, undo_req).await;
        assert!(undo_resp.status().is_success());
    }

    let content_after_undo = std::fs::read_to_string(&note_abs).unwrap();
    assert!(!content_after_undo.contains("persisted"));
}
