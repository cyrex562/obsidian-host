use actix_web::{http::header, test, web, App};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use codex::config::AppConfig;
use codex::db::Database;
use codex::middleware::AuthMiddleware;
use codex::models::{CreateFileRequest, CreateVaultRequest, UpdateFileRequest};
use codex::routes::{auth, files, vaults, AppState};
use codex::services::{MarkdownParser, SearchIndex};
use codex::watcher::FileWatcher;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{broadcast, Mutex};

fn password_hash(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

#[actix_web::test]
async fn verify_etag_optimistic_locking_and_change_log() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("sync-test.db");
    let db_url = format!("sqlite://{}", db_path.display());
    let db = Database::new(&db_url).await.unwrap();

    db.bootstrap_admin_if_empty(Some("admin"), Some("hunter2"))
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
        ml_undo_store: std::sync::Arc::new(tokio::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        shutdown_tx: tokio::sync::broadcast::channel::<()>(1).0,
        document_parser: Arc::new(MarkdownParser),
        entity_type_registry: codex::services::EntityTypeRegistry::new(),
        relation_type_registry: codex::services::RelationTypeRegistry::new(),
        plugins_dir: String::new(),
    });

    let mut config = AppConfig::default();
    config.auth.enabled = true;
    config.auth.jwt_secret = "sync-test-secret".to_string();
    let config = web::Data::new(config);

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .app_data(config.clone())
            .wrap(AuthMiddleware)
            .configure(auth::configure)
            .configure(vaults::configure)
            .configure(files::configure),
    )
    .await;

    // Login
    let login_req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({ "username": "admin", "password": "hunter2" }))
        .to_request();
    let login_resp = test::call_service(&app, login_req).await;
    let login_body: serde_json::Value = test::read_body_json(login_resp).await;
    let token = login_body["access_token"].as_str().unwrap().to_string();
    let auth_header = format!("Bearer {}", token);

    // Create a vault
    let vault_dir = temp_dir.path().join("test-vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let create_vault_req = test::TestRequest::post()
        .uri("/api/vaults")
        .insert_header((header::AUTHORIZATION, auth_header.clone()))
        .set_json(&CreateVaultRequest {
            name: "Sync Vault".to_string(),
            path: Some(vault_dir.to_string_lossy().to_string()),
        })
        .to_request();

    let create_vault_resp = test::call_service(&app, create_vault_req).await;
    let vault_body: serde_json::Value = test::read_body_json(create_vault_resp).await;
    let vault_id = vault_body["id"].as_str().unwrap().to_string();

    // 1. Create a file
    let create_req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/files", vault_id))
        .insert_header((header::AUTHORIZATION, auth_header.clone()))
        .set_json(&CreateFileRequest {
            path: "note.md".to_string(),
            content: Some("First Content".to_string()),
        })
        .to_request();
    let create_resp = test::call_service(&app, create_req).await;
    assert!(create_resp.status().is_success());
    let original_etag_str = create_resp
        .headers()
        .get(header::ETAG)
        .expect("create_file must return an ETag header")
        .to_str()
        .unwrap()
        .to_string();
    let _: serde_json::Value = test::read_body_json(create_resp).await;

    // 2. Fetch changes API
    let changes_req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{}/changes", vault_id))
        .insert_header((header::AUTHORIZATION, auth_header.clone()))
        .to_request();
    let changes_resp = test::call_service(&app, changes_req).await;
    assert!(changes_resp.status().is_success());
    let changes_body: serde_json::Value = test::read_body_json(changes_resp).await;
    let changes_array = changes_body.as_array().unwrap();
    assert_eq!(changes_array.len(), 1);
    assert_eq!(changes_array[0]["event_type"].as_str().unwrap(), "created");

    // 3. Update file perfectly using ETag
    let update_req = test::TestRequest::put()
        .uri(&format!("/api/vaults/{}/files/note.md", vault_id))
        .insert_header((header::AUTHORIZATION, auth_header.clone()))
        .insert_header((header::IF_MATCH, original_etag_str.clone()))
        .set_json(&UpdateFileRequest {
            content: "Second Content".to_string(),
            last_modified: None,
            frontmatter: None,
        })
        .to_request();
    let update_resp = test::call_service(&app, update_req).await;
    assert!(update_resp.status().is_success());
    let _second_etag_str = update_resp
        .headers()
        .get(header::ETAG)
        .expect("update_file must return an ETag header")
        .to_str()
        .unwrap()
        .to_string();
    let _: serde_json::Value = test::read_body_json(update_resp).await;

    // 4. Stale write (Conflict) using OLD ETag
    let conflict_req = test::TestRequest::put()
        .uri(&format!("/api/vaults/{}/files/note.md", vault_id))
        .insert_header((header::AUTHORIZATION, auth_header.clone()))
        .insert_header((header::IF_MATCH, original_etag_str.clone()))
        .set_json(&UpdateFileRequest {
            content: "Conflicting Content".to_string(),
            last_modified: None,
            frontmatter: None,
        })
        .to_request();
    let conflict_resp = test::call_service(&app, conflict_req).await;
    assert_eq!(conflict_resp.status().as_u16(), 412); // Precondition Failed
    let conflict_body: serde_json::Value = test::read_body_json(conflict_resp).await;
    assert_eq!(
        conflict_body["error"].as_str().unwrap(),
        "precondition_failed"
    );
    assert_eq!(
        conflict_body["server_content"]["content"].as_str().unwrap(),
        "Second Content"
    );
}
