use actix_web::{test, web, App};
use obsidian_host::db::Database;
use obsidian_host::models::{CreateFileRequest, UpdateFileRequest};
use obsidian_host::routes::{files, vaults, AppState};
use obsidian_host::services::{FileService, SearchIndex};
use obsidian_host::watcher::FileWatcher;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{broadcast, Mutex};

async fn setup_app_state(temp_dir: &TempDir) -> (web::Data<AppState>, String) {
    let db_path = temp_dir.path().join("test.db");
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
    });

    // Create a vault
    let vault_path = temp_dir.path().join("vault");
    std::fs::create_dir_all(&vault_path).unwrap();
    let vault = db
        .create_vault(
            "Test Vault".to_string(),
            vault_path.to_string_lossy().to_string(),
        )
        .await
        .unwrap();

    (state, vault.id)
}

#[actix_web::test]
async fn test_create_file_api() {
    let temp_dir = TempDir::new().unwrap();
    let (app_state, vault_id) = setup_app_state(&temp_dir).await;

    let app = test::init_service(
        App::new()
            .app_data(app_state.clone())
            .configure(files::configure),
    )
    .await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/files", vault_id))
        .set_json(&CreateFileRequest {
            path: "test_note.md".to_string(),
            content: Some("# Test Note".to_string()),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Verify file exists on disk
    let vault_path = temp_dir.path().join("vault");
    let file_path = vault_path.join("test_note.md");
    assert!(file_path.exists());
    let content = std::fs::read_to_string(file_path).unwrap();
    assert_eq!(content, "# Test Note");
}

#[actix_web::test]
async fn test_read_file_api() {
    let temp_dir = TempDir::new().unwrap();
    let (app_state, vault_id) = setup_app_state(&temp_dir).await;

    let app = test::init_service(
        App::new()
            .app_data(app_state.clone())
            .configure(files::configure),
    )
    .await;

    // Create file manually
    let vault_path = temp_dir.path().join("vault");
    let file_path = vault_path.join("read_test.md");
    std::fs::write(&file_path, "Read content").unwrap();

    let req = test::TestRequest::get()
        .uri(&format!("/api/vaults/{}/files/read_test.md", vault_id))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Check response body
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["content"], "Read content");
    assert_eq!(body["path"], "read_test.md");
}

#[actix_web::test]
async fn test_update_file_api() {
    let temp_dir = TempDir::new().unwrap();
    let (app_state, vault_id) = setup_app_state(&temp_dir).await;

    let app = test::init_service(
        App::new()
            .app_data(app_state.clone())
            .configure(files::configure),
    )
    .await;

    // Create file manually
    let vault_path = temp_dir.path().join("vault");
    let file_path = vault_path.join("update_test.md");
    std::fs::write(&file_path, "Initial content").unwrap();

    // Get modified time to update safely
    let metadata = std::fs::metadata(&file_path).unwrap();
    let modified = metadata.modified().unwrap();

    let req = test::TestRequest::put()
        .uri(&format!("/api/vaults/{}/files/update_test.md", vault_id))
        .set_json(&UpdateFileRequest {
            content: "Updated content".to_string(),
            last_modified: None, // Force update for this test simplicity
            frontmatter: None,
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Verify file updated on disk
    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Updated content");
}

#[actix_web::test]
async fn test_delete_file_api() {
    let temp_dir = TempDir::new().unwrap();
    let (app_state, vault_id) = setup_app_state(&temp_dir).await;

    let app = test::init_service(
        App::new()
            .app_data(app_state.clone())
            .configure(files::configure),
    )
    .await;

    // Create file manually
    let vault_path = temp_dir.path().join("vault");
    let file_path = vault_path.join("delete_test.md");
    std::fs::write(&file_path, "Delete me").unwrap();

    let req = test::TestRequest::delete()
        .uri(&format!("/api/vaults/{}/files/delete_test.md", vault_id))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 204);

    // Verify file deleted (or moved to trash depending on implementation, here check it's gone from original location)
    assert!(!file_path.exists());
    let trash_dir = temp_dir.path().join("vault").join(".trash");
    assert!(trash_dir.exists());

    // Check if any file in trash ends with "delete_test.md"
    let mut found = false;
    for entry in std::fs::read_dir(trash_dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with("delete_test.md") {
            found = true;
            break;
        }
    }
    assert!(found, "File not found in trash");
}

#[actix_web::test]
async fn test_create_directory_api() {
    let temp_dir = TempDir::new().unwrap();
    let (app_state, vault_id) = setup_app_state(&temp_dir).await;

    let app = test::init_service(
        App::new()
            .app_data(app_state.clone())
            .configure(files::configure),
    )
    .await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/directories", vault_id))
        .set_json(&CreateFileRequest {
            path: "new_folder".to_string(),
            content: None,
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Verify directory exists
    let vault_path = temp_dir.path().join("vault");
    let dir_path = vault_path.join("new_folder");
    assert!(dir_path.exists());
    assert!(dir_path.is_dir());
}

#[actix_web::test]
async fn test_rename_file_api() {
    let temp_dir = TempDir::new().unwrap();
    let (app_state, vault_id) = setup_app_state(&temp_dir).await;

    let app = test::init_service(
        App::new()
            .app_data(app_state.clone())
            .configure(files::configure),
    )
    .await;

    // Create file manually
    let vault_path = temp_dir.path().join("vault");
    let file_path = vault_path.join("old_name.md");
    std::fs::write(&file_path, "Rename me").unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/vaults/{}/rename", vault_id))
        .set_json(&json!({
            "from": "old_name.md",
            "to": "new_name.md"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Verify rename
    assert!(!file_path.exists());
    let new_path = vault_path.join("new_name.md");
    assert!(new_path.exists());
    let content = std::fs::read_to_string(new_path).unwrap();
    assert_eq!(content, "Rename me");
}
