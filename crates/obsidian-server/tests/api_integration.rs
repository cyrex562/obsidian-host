use actix_web::{http::header, test, web, App};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use obsidian_host::config::AppConfig;
use obsidian_host::db::Database;
use obsidian_host::middleware::AuthMiddleware;
use obsidian_host::routes::{auth, api_keys, files, vaults, AppState};
use obsidian_host::services::{default_storage_backend, SearchIndex};
use obsidian_host::watcher::FileWatcher;
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
async fn verify_api_keys_and_totp() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("api-test.db");
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
        storage: default_storage_backend(),
        watcher,
        event_broadcaster: event_tx,
        change_log_retention_days: 7,
        ml_undo_store: std::sync::Arc::new(tokio::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
    });

    let mut config = AppConfig::default();
    config.auth.enabled = true;
    config.auth.jwt_secret = "api-test-secret".to_string();
    let config = web::Data::new(config);

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .app_data(config.clone())
            .wrap(AuthMiddleware)
            .configure(auth::configure)
            .configure(api_keys::configure),
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

    // 1. Generate API Key
    let create_key_req = test::TestRequest::post()
        .uri("/api/auth/api-keys")
        .insert_header((header::AUTHORIZATION, auth_header.clone()))
        .set_json(json!({ "name": "Desktop Client Key", "expires_in_days": 30 }))
        .to_request();
    let create_key_resp = test::call_service(&app, create_key_req).await;
    assert!(create_key_resp.status().is_success());
    let create_key_body: serde_json::Value = test::read_body_json(create_key_resp).await;
    let api_key = create_key_body["api_key"].as_str().unwrap().to_string();
    let prefix = create_key_body["prefix"].as_str().unwrap().to_string();
    let id = create_key_body["id"].as_str().unwrap().to_string();

    // 2. Validate API Key by calling /api/auth/me bypassing JWT auth
    let me_req = test::TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(("X-API-Key", api_key.clone()))
        .to_request();
    let me_resp = test::call_service(&app, me_req).await;
    let me_status = me_resp.status();
    let me_body: serde_json::Value = test::read_body_json(me_resp).await;
    assert!(me_status.is_success(), "Failed to use API key. Status: {}, Body: {}", me_status, me_body);
    assert_eq!(me_body["username"], "admin");

    // 3. List API Keys
    let list_req = test::TestRequest::get()
        .uri("/api/auth/api-keys")
        .insert_header((header::AUTHORIZATION, auth_header.clone()))
        .to_request();
    let list_resp = test::call_service(&app, list_req).await;
    assert!(list_resp.status().is_success());
    let list_body: serde_json::Value = test::read_body_json(list_resp).await;
    let keys_array = list_body.as_array().unwrap();
    assert_eq!(keys_array.len(), 1);
    assert_eq!(keys_array[0]["prefix"].as_str().unwrap(), prefix);

    // 4. Revoke API Key
    let revoke_req = test::TestRequest::delete()
        .uri(&format!("/api/auth/api-keys/{}", id))
        .insert_header((header::AUTHORIZATION, auth_header.clone()))
        .to_request();
    let revoke_resp = test::call_service(&app, revoke_req).await;
    assert!(revoke_resp.status().is_success());

    // 5. Try using revoked Key (Should fail)
    let bad_me_req = test::TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(("X-API-Key", api_key))
        .to_request();
    let bad_me_resp = test::call_service(&app, bad_me_req).await;
    assert_eq!(bad_me_resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_public_vault_allows_anonymous_reads() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("pub-vault-test.db");
    let db_url = format!("sqlite://{}", db_path.display());
    let db = Database::new(&db_url).await.unwrap();

    db.bootstrap_admin_if_empty(Some("admin"), Some("secret123"))
        .await
        .unwrap();

    let vault_root = temp_dir.path().join("vaults");
    std::fs::create_dir_all(&vault_root).unwrap();

    // ── Build app ─────────────────────────────────────────────────────────
    let search_index = SearchIndex::new();
    let (watcher, _) = FileWatcher::new().unwrap();
    let watcher = Arc::new(Mutex::new(watcher));
    let (event_tx, _) = broadcast::channel(100);

    let state = web::Data::new(AppState {
        db: db.clone(),
        search_index,
        storage: default_storage_backend(),
        watcher,
        event_broadcaster: event_tx,
        change_log_retention_days: 7,
        ml_undo_store: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    });

    let mut config = AppConfig::default();
    config.auth.enabled = true;
    config.auth.jwt_secret = "pub-test-secret".to_string();
    config.vault.base_dir = vault_root.to_string_lossy().to_string();
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

    // ── Login to get a token ──────────────────────────────────────────────
    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/login")
            .set_json(json!({ "username": "admin", "password": "secret123" }))
            .to_request(),
    )
    .await;
    let login_body: serde_json::Value = test::read_body_json(login_resp).await;
    let token = login_body["access_token"].as_str().unwrap().to_string();
    let auth_header = format!("Bearer {token}");

    // ── Create a vault ────────────────────────────────────────────────────
    let vault_path = vault_root.join("testvault");
    std::fs::create_dir_all(&vault_path).unwrap();

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/vaults")
            .insert_header((header::AUTHORIZATION, auth_header.clone()))
            .set_json(json!({
                "name": "Test Vault",
                "path": vault_path.to_string_lossy()
            }))
            .to_request(),
    )
    .await;
    assert!(create_resp.status().is_success(), "vault creation failed");
    let vault_body: serde_json::Value = test::read_body_json(create_resp).await;
    let vault_id = vault_body["id"].as_str().unwrap().to_string();

    // ── Unauthenticated GET on a private vault → 401 ─────────────────────
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/vaults/{vault_id}/files"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 401, "private vault must reject anonymous reads");

    // ── Mark vault as public ─────────────────────────────────────────────
    let vis_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/vaults/{vault_id}/visibility"))
            .insert_header((header::AUTHORIZATION, auth_header.clone()))
            .set_json(json!({ "visibility": "public" }))
            .to_request(),
    )
    .await;
    assert!(vis_resp.status().is_success(), "setting visibility failed");

    // ── Unauthenticated GET on a public vault → 200 ──────────────────────
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/vaults/{vault_id}/files"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 200, "public vault must allow anonymous reads");

    // ── Unauthenticated write on a public vault → still 401 ──────────────
    let resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/vaults/{vault_id}/files/note.md"))
            .set_payload("# Hello")
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 401, "public vault must reject anonymous writes");

    // ── Revert to private ────────────────────────────────────────────────
    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/vaults/{vault_id}/visibility"))
            .insert_header((header::AUTHORIZATION, auth_header.clone()))
            .set_json(json!({ "visibility": "private" }))
            .to_request(),
    )
    .await;

    // ── Unauthenticated GET after reverting to private → 401 again ────────
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/vaults/{vault_id}/files"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 401, "reverted private vault must reject anonymous reads");

    let _ = state; // keep state alive
}
