use actix_web::{test, App};
use obsidian_host::routes::sync;
use obsidian_host::services::{AuthService, PluginService, SearchIndex};
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;
use zip::write::FileOptions;
use obsidian_host::routes::AppState;

#[actix_web::test]
async fn test_upload_zip() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}", db_path.to_str().unwrap());
    
    // Create DB
    let db = obsidian_host::db::Database::new(&db_url).await.unwrap();
    
    // Create SearchIndex
    let search_index = SearchIndex::new();
    
    // Create a ZIP file
    let zip_path = temp_dir.path().join("test_vault.zip");
    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    
    let options = FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);

    zip.start_file("hello.md", options).unwrap();
    zip.write_all(b"# Hello World\nThis is a test.").unwrap();
    
    zip.start_file("folder/nested.md", options).unwrap();
    zip.write_all(b"# Nested\nMarkdown file.").unwrap();
    
    zip.finish().unwrap();
    
    // Read ZIP content
    let zip_content = std::fs::read(&zip_path).unwrap();
    
    // Construct multipart body manually
    let boundary = "------------------------Boundary123";
    let body_start = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test_vault.zip\"\r\nContent-Type: application/zip\r\n\r\n",
        boundary = boundary
    );
    let body_end = format!("\r\n--{boundary}--\r\n", boundary = boundary);
    
    let mut payload = Vec::new();
    payload.extend_from_slice(body_start.as_bytes());
    payload.extend_from_slice(&zip_content);
    payload.extend_from_slice(body_end.as_bytes());

    // Init App
    // Mock Config
    let mut config = obsidian_host::config::AppConfig::default();
    config.vault.root_dir = temp_dir.path().join("vaults");
    std::fs::create_dir_all(&config.vault.root_dir).unwrap();
    
    // Create a user in DB so we can authenticate
    let user = db.upsert_user_from_oidc("test@example.com", "Test User", None, "sub123", "iss").await.unwrap();
    let session = db.create_session(&user.id, "dummy_token_hash", 24).await.unwrap();
    
    // Create App State
    let app_state = actix_web::web::Data::new(AppState {
        db: db.clone(),
        search_index: search_index.clone(),
        watcher: std::sync::Arc::new(tokio::sync::Mutex::new(obsidian_host::watcher::FileWatcher::new().unwrap().0)),
        event_broadcaster: tokio::sync::broadcast::channel(10).0,
        auth_service: None, 
        plugin_service: std::sync::Arc::new(tokio::sync::RwLock::new(obsidian_host::services::PluginService::new(temp_dir.path().join("plugins")))),
        force_secure_cookies: false,
        config: config.clone(),
    });
    
    let app = test::init_service(
        App::new()
            .app_data(app_state)
            .configure(sync::config)
    ).await;
    
    // Perform Request
    let req = test::TestRequest::post()
        .uri("/test-vault/upload") 
        .insert_header(("content-type", format!("multipart/form-data; boundary={}", boundary)))
        .cookie(actix_web::cookie::Cookie::build("session_id", session.id).path("/").finish())
        .set_payload(payload)
        .to_request();
        
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "Upload failed with status: {}", resp.status());
    
    // Verify files extracted
    let vault_dir = config.vault.root_dir.join("test-vault");
    assert!(vault_dir.exists(), "Vault directory not created");
    assert!(vault_dir.join("hello.md").exists(), "hello.md not extracted");
    assert!(vault_dir.join("folder/nested.md").exists(), "nested.md not extracted");
    
    // Verify search index
    let results = search_index.search("test-vault", "hello", 1, 10).unwrap();
    assert!(!results.results.is_empty(), "Search index not updated");
    assert_eq!(results.results[0].path, "hello.md");
}
