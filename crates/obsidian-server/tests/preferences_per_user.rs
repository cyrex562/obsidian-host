use actix_web::{http::header, test, web, App};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use obsidian_host::config::AppConfig;
use obsidian_host::db::Database;
use obsidian_host::middleware::AuthMiddleware;
use obsidian_host::routes::{auth, preferences, AppState};
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
async fn preferences_are_scoped_per_authenticated_user() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("prefs-per-user.db");
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
        storage: default_storage_backend(),
        watcher,
        event_broadcaster: event_tx,
        change_log_retention_days: 7,
    });

    let mut config = AppConfig::default();
    config.auth.enabled = true;
    config.auth.jwt_secret = "preferences-test-secret".to_string();
    let config = web::Data::new(config);

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .app_data(config.clone())
            .wrap(AuthMiddleware)
            .configure(auth::configure)
            .configure(preferences::configure),
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

    let admin_get_before = test::TestRequest::get()
        .uri("/api/preferences")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .to_request();
    let admin_before_resp = test::call_service(&app, admin_get_before).await;
    assert!(admin_before_resp.status().is_success());
    let admin_before: serde_json::Value = test::read_body_json(admin_before_resp).await;
    assert_eq!(admin_before["theme"], "dark");

    let alice_get_before = test::TestRequest::get()
        .uri("/api/preferences")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", alice_token)))
        .to_request();
    let alice_before_resp = test::call_service(&app, alice_get_before).await;
    assert!(alice_before_resp.status().is_success());
    let alice_before: serde_json::Value = test::read_body_json(alice_before_resp).await;
    assert_eq!(alice_before["theme"], "dark");

    let admin_put = test::TestRequest::put()
        .uri("/api/preferences")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .set_json(json!({
            "theme": "light",
            "editor_mode": "side_by_side",
            "font_size": 16,
            "window_layout": null
        }))
        .to_request();
    let admin_put_resp = test::call_service(&app, admin_put).await;
    assert!(admin_put_resp.status().is_success());

    let admin_get_after = test::TestRequest::get()
        .uri("/api/preferences")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", admin_token)))
        .to_request();
    let admin_after_resp = test::call_service(&app, admin_get_after).await;
    assert!(admin_after_resp.status().is_success());
    let admin_after: serde_json::Value = test::read_body_json(admin_after_resp).await;
    assert_eq!(admin_after["theme"], "light");
    assert_eq!(admin_after["font_size"], 16);

    let alice_get_after = test::TestRequest::get()
        .uri("/api/preferences")
        .insert_header((header::AUTHORIZATION, format!("Bearer {}", alice_token)))
        .to_request();
    let alice_after_resp = test::call_service(&app, alice_get_after).await;
    assert!(alice_after_resp.status().is_success());
    let alice_after: serde_json::Value = test::read_body_json(alice_after_resp).await;
    assert_eq!(alice_after["theme"], "dark");
    assert_eq!(alice_after["font_size"], 14);
}
