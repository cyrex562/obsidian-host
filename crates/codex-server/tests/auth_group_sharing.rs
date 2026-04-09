use actix_web::{http::header, test, web, App};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use codex::config::AppConfig;
use codex::db::Database;
use codex::middleware::AuthMiddleware;
use codex::models::{CreateGroupRequest, CreateVaultRequest};
use codex::routes::{auth, groups, vaults, AppState};
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
async fn username_login_group_membership_and_vault_sharing_work() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("auth-sharing.db");
    let db_url = format!("sqlite://{}", db_path.display());
    let db = Database::new(&db_url).await.unwrap();

    db.bootstrap_admin_if_empty(Some("admin"), Some("hunter2"))
        .await
        .unwrap();
    db.create_user("alice", &password_hash("password123"))
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
    config.auth.jwt_secret = "integration-test-secret".to_string();
    let config = web::Data::new(config);

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .app_data(config.clone())
            .wrap(AuthMiddleware)
            .configure(auth::configure)
            .configure(groups::configure)
            .configure(vaults::configure),
    )
    .await;

    let admin_login_req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({ "username": "admin", "password": "hunter2" }))
        .to_request();
    let admin_login_resp = test::call_service(&app, admin_login_req).await;
    assert!(admin_login_resp.status().is_success());
    let admin_login_body: serde_json::Value = test::read_body_json(admin_login_resp).await;
    let admin_token = admin_login_body["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let alice_login_req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({ "username": "alice", "password": "password123" }))
        .to_request();
    let alice_login_resp = test::call_service(&app, alice_login_req).await;
    assert!(alice_login_resp.status().is_success());
    let alice_login_body: serde_json::Value = test::read_body_json(alice_login_resp).await;
    let alice_token = alice_login_body["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let me_req = test::TestRequest::get()
        .uri("/api/auth/me")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .to_request();
    let me_resp = test::call_service(&app, me_req).await;
    assert!(me_resp.status().is_success());
    let me_body: serde_json::Value = test::read_body_json(me_resp).await;
    assert_eq!(me_body["username"], "admin");
    assert_eq!(me_body["auth_method"], "password");

    let vault_dir = temp_dir.path().join("shared-vault");
    std::fs::create_dir_all(&vault_dir).unwrap();
    let create_vault_req = test::TestRequest::post()
        .uri("/api/vaults")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .set_json(&CreateVaultRequest {
            name: "Shared Vault".to_string(),
            path: Some(vault_dir.to_string_lossy().to_string()),
        })
        .to_request();
    let create_vault_resp = test::call_service(&app, create_vault_req).await;
    assert!(create_vault_resp.status().is_success());
    let vault_body: serde_json::Value = test::read_body_json(create_vault_resp).await;
    let vault_id = vault_body["id"].as_str().unwrap().to_string();

    let alice_vaults_req = test::TestRequest::get()
        .uri("/api/vaults")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", alice_token)))
        .to_request();
    let alice_vaults_resp = test::call_service(&app, alice_vaults_req).await;
    assert!(alice_vaults_resp.status().is_success());
    let alice_vaults: serde_json::Value = test::read_body_json(alice_vaults_resp).await;
    assert_eq!(alice_vaults.as_array().unwrap().len(), 0);

    let create_group_req = test::TestRequest::post()
        .uri("/api/groups")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .set_json(&CreateGroupRequest {
            name: "editors".to_string(),
        })
        .to_request();
    let create_group_resp = test::call_service(&app, create_group_req).await;
    assert!(create_group_resp.status().is_success());
    let group_body: serde_json::Value = test::read_body_json(create_group_resp).await;
    let group_id = group_body["id"].as_str().unwrap().to_string();

    let alice_user_id = db
        .get_user_by_username("alice")
        .await
        .unwrap()
        .map(|(id, _)| id)
        .unwrap();

    let add_member_req = test::TestRequest::post()
        .uri(&format!("/api/groups/{}/members", group_id))
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .set_json(json!({ "user_id": alice_user_id }))
        .to_request();
    let add_member_resp = test::call_service(&app, add_member_req).await;
    assert!(add_member_resp.status().is_success());

    let share_group_req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/shares/groups", vault_id))
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .set_json(json!({ "group_id": group_id, "role": "viewer" }))
        .to_request();
    let share_group_resp = test::call_service(&app, share_group_req).await;
    assert!(share_group_resp.status().is_success());

    let alice_vaults_req = test::TestRequest::get()
        .uri("/api/vaults")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", alice_token)))
        .to_request();
    let alice_vaults_resp = test::call_service(&app, alice_vaults_req).await;
    assert!(alice_vaults_resp.status().is_success());
    let alice_vaults: serde_json::Value = test::read_body_json(alice_vaults_resp).await;
    assert_eq!(alice_vaults.as_array().unwrap().len(), 1);
    assert_eq!(alice_vaults[0]["name"], "Shared Vault");

    let revoke_group_share_req = test::TestRequest::delete()
        .uri(&format!(
            "/api/vaults/{}/shares/groups/{}",
            vault_id, group_id
        ))
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .to_request();
    let revoke_group_share_resp = test::call_service(&app, revoke_group_share_req).await;
    assert!(revoke_group_share_resp.status().is_success());

    let alice_vaults_req = test::TestRequest::get()
        .uri("/api/vaults")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", alice_token)))
        .to_request();
    let alice_vaults_resp = test::call_service(&app, alice_vaults_req).await;
    assert!(alice_vaults_resp.status().is_success());
    let alice_vaults: serde_json::Value = test::read_body_json(alice_vaults_resp).await;
    assert_eq!(alice_vaults.as_array().unwrap().len(), 0);

    let share_user_req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/shares/users", vault_id))
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .set_json(json!({ "user_id": alice_user_id, "role": "viewer" }))
        .to_request();
    let share_user_resp = test::call_service(&app, share_user_req).await;
    assert!(share_user_resp.status().is_success());

    let alice_vaults_req = test::TestRequest::get()
        .uri("/api/vaults")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", alice_token)))
        .to_request();
    let alice_vaults_resp = test::call_service(&app, alice_vaults_req).await;
    assert!(alice_vaults_resp.status().is_success());
    let alice_vaults: serde_json::Value = test::read_body_json(alice_vaults_resp).await;
    assert_eq!(alice_vaults.as_array().unwrap().len(), 1);

    let revoke_user_share_req = test::TestRequest::delete()
        .uri(&format!(
            "/api/vaults/{}/shares/users/{}",
            vault_id, alice_user_id
        ))
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .to_request();
    let revoke_user_share_resp = test::call_service(&app, revoke_user_share_req).await;
    assert!(revoke_user_share_resp.status().is_success());

    let alice_vaults_req = test::TestRequest::get()
        .uri("/api/vaults")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", alice_token)))
        .to_request();
    let alice_vaults_resp = test::call_service(&app, alice_vaults_req).await;
    assert!(alice_vaults_resp.status().is_success());
    let alice_vaults: serde_json::Value = test::read_body_json(alice_vaults_resp).await;
    assert_eq!(alice_vaults.as_array().unwrap().len(), 0);

    let delete_req = test::TestRequest::delete()
        .uri(&format!("/api/vaults/{}", vault_id))
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", alice_token)))
        .to_request();
    let delete_resp = test::call_service(&app, delete_req).await;
    assert_eq!(delete_resp.status().as_u16(), 403);
}
