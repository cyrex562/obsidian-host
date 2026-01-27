use actix_web::{test, web, App};
use obsidian_host::db::Database;
use obsidian_host::models::CreateVaultRequest;
use obsidian_host::routes::{vaults, AppState};
use obsidian_host::services::SearchIndex;
use obsidian_host::watcher::FileWatcher;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{broadcast, Mutex};

async fn setup_app_state(temp_dir: &TempDir) -> web::Data<AppState> {
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}", db_path.display());
    let db = Database::new(&db_url).await.unwrap();

    let search_index = SearchIndex::new();
    let (watcher, _) = FileWatcher::new().unwrap();
    let watcher = Arc::new(Mutex::new(watcher));
    let (event_tx, _) = broadcast::channel(100);

    web::Data::new(AppState {
        db: db.clone(),
        search_index,
        watcher,
        event_broadcaster: event_tx,
    })
}

#[actix_web::test]
async fn test_create_and_list_vaults() {
    let temp_dir = TempDir::new().unwrap();
    let app_state = setup_app_state(&temp_dir).await;

    let app = test::init_service(
        App::new()
            .app_data(app_state.clone())
            .configure(vaults::configure),
    )
    .await;

    // Create a physical directory for the vault
    let vault_dir = temp_dir.path().join("vault1");
    std::fs::create_dir(&vault_dir).unwrap();

    // 1. Create Vault (Task 1.1 Add Vault)
    let req = test::TestRequest::post()
        .uri("/api/vaults")
        .set_json(&CreateVaultRequest {
            name: "Vault 1".to_string(),
            path: vault_dir.to_string_lossy().to_string(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let vault: serde_json::Value = test::read_body_json(resp).await;
    let vault_id = vault["id"].as_str().unwrap();
    assert_eq!(vault["name"], "Vault 1");

    // 2. List Vaults (Task 1.1 Verify vault appears)
    let req = test::TestRequest::get().uri("/api/vaults").to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let vaults: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert_eq!(vaults.len(), 1);
    assert_eq!(vaults[0]["id"], vault_id);
    assert_eq!(vaults[0]["name"], "Vault 1");
}

#[actix_web::test]
async fn test_switch_vaults() {
    let temp_dir = TempDir::new().unwrap();
    let app_state = setup_app_state(&temp_dir).await;

    let app = test::init_service(
        App::new()
            .app_data(app_state.clone())
            .configure(vaults::configure),
    )
    .await;

    // Create directories
    let vault1_dir = temp_dir.path().join("vault1");
    let vault2_dir = temp_dir.path().join("vault2");
    std::fs::create_dir(&vault1_dir).unwrap();
    std::fs::create_dir(&vault2_dir).unwrap();

    // Add Vault 1
    let req = test::TestRequest::post()
        .uri("/api/vaults")
        .set_json(&CreateVaultRequest {
            name: "Vault A".to_string(),
            path: vault1_dir.to_string_lossy().to_string(),
        })
        .to_request();
    let _ = test::call_service(&app, req).await;

    // Add Vault 2
    let req = test::TestRequest::post()
        .uri("/api/vaults")
        .set_json(&CreateVaultRequest {
            name: "Vault B".to_string(),
            path: vault2_dir.to_string_lossy().to_string(),
        })
        .to_request();
    let _ = test::call_service(&app, req).await;

    // List to verify both exist
    let req = test::TestRequest::get().uri("/api/vaults").to_request();
    let resp = test::call_service(&app, req).await;
    let vaults: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert_eq!(vaults.len(), 2);

    // Validate we can retrieve each vault individually (mimics switching context)
    let id_a = vaults.iter().find(|v| v["name"] == "Vault A").unwrap()["id"]
        .as_str()
        .unwrap();
    let id_b = vaults.iter().find(|v| v["name"] == "Vault B").unwrap()["id"]
        .as_str()
        .unwrap();

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{}", id_a))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{}", id_b))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}
