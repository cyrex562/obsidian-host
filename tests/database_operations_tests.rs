use obsidian_host::db::Database;
use obsidian_host::models::EditorMode;
use tempfile::TempDir;

#[tokio::test]
async fn test_database_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    // Verify database file was created
    assert!(db_path.exists());

    // Verify we can query the database
    let vaults = db.list_vaults().await.unwrap();
    assert_eq!(vaults.len(), 0);
}

#[tokio::test]
async fn test_vault_crud_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    let vault_dir = TempDir::new().unwrap();
    let vault_path = vault_dir.path().to_str().unwrap();

    // CREATE
    let vault = db
        .create_vault("Test Vault".to_string(), vault_path.to_string())
        .await
        .unwrap();

    assert_eq!(vault.name, "Test Vault");
    assert_eq!(vault.path, vault_path);
    assert!(!vault.id.is_empty());

    // READ
    let retrieved = db.get_vault(&vault.id).await.unwrap();
    assert_eq!(retrieved.id, vault.id);
    assert_eq!(retrieved.name, vault.name);
    assert_eq!(retrieved.path, vault.path);

    // LIST
    let all_vaults = db.list_vaults().await.unwrap();
    assert_eq!(all_vaults.len(), 1);
    assert_eq!(all_vaults[0].id, vault.id);

    // DELETE
    db.delete_vault(&vault.id).await.unwrap();

    let result = db.get_vault(&vault.id).await;
    assert!(result.is_err());

    let remaining = db.list_vaults().await.unwrap();
    assert_eq!(remaining.len(), 0);
}

#[tokio::test]
async fn test_vault_duplicate_prevention() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    let vault_dir = TempDir::new().unwrap();
    let vault_path = vault_dir.path().to_str().unwrap();

    // Create first vault
    db.create_vault("Vault 1".to_string(), vault_path.to_string())
        .await
        .unwrap();

    // Attempt to create vault with same path should fail
    let result = db
        .create_vault("Vault 2".to_string(), vault_path.to_string())
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_recent_files_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    let vault_dir = TempDir::new().unwrap();
    let vault = db
        .create_vault(
            "Test Vault".to_string(),
            vault_dir.path().to_str().unwrap().to_string(),
        )
        .await
        .unwrap();

    // Record recent files
    db.record_recent_file(&vault.id, "file1.md").await.unwrap();
    db.record_recent_file(&vault.id, "file2.md").await.unwrap();
    db.record_recent_file(&vault.id, "file3.md").await.unwrap();

    // Get recent files
    let recent = db.get_recent_files(&vault.id, 10).await.unwrap();
    assert_eq!(recent.len(), 3);

    // Most recent should be first
    assert_eq!(recent[0], "file3.md");
    assert_eq!(recent[1], "file2.md");
    assert_eq!(recent[2], "file1.md");

    // Record duplicate - should update timestamp
    db.record_recent_file(&vault.id, "file1.md").await.unwrap();

    let recent_after = db.get_recent_files(&vault.id, 10).await.unwrap();
    assert_eq!(recent_after.len(), 3);
    assert_eq!(recent_after[0], "file1.md"); // Now first
}

#[tokio::test]
async fn test_recent_files_limit() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    let vault_dir = TempDir::new().unwrap();
    let vault = db
        .create_vault(
            "Test Vault".to_string(),
            vault_dir.path().to_str().unwrap().to_string(),
        )
        .await
        .unwrap();

    // Record 25 files
    for i in 1..=25 {
        db.record_recent_file(&vault.id, &format!("file{}.md", i))
            .await
            .unwrap();
    }

    // Should only keep 20 most recent
    let recent = db.get_recent_files(&vault.id, 25).await.unwrap();
    assert_eq!(recent.len(), 20);

    // Most recent should be file25
    assert_eq!(recent[0], "file25.md");
    // Oldest should be file6
    assert_eq!(recent[19], "file6.md");
}

#[tokio::test]
async fn test_recent_files_cascade_delete() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    let vault_dir = TempDir::new().unwrap();
    let vault = db
        .create_vault(
            "Test Vault".to_string(),
            vault_dir.path().to_str().unwrap().to_string(),
        )
        .await
        .unwrap();

    // Record recent files
    db.record_recent_file(&vault.id, "file1.md").await.unwrap();
    db.record_recent_file(&vault.id, "file2.md").await.unwrap();

    let recent_before = db.get_recent_files(&vault.id, 10).await.unwrap();
    assert_eq!(recent_before.len(), 2);

    // Delete vault
    db.delete_vault(&vault.id).await.unwrap();

    // Recent files should be deleted (cascade)
    let recent_after = db.get_recent_files(&vault.id, 10).await.unwrap();
    assert_eq!(recent_after.len(), 0);
}

#[tokio::test]
async fn test_preferences_crud_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    // Get default preferences
    let prefs = db.get_preferences().await.unwrap();
    assert_eq!(prefs.theme, "dark");
    assert_eq!(prefs.editor_mode, EditorMode::SideBySide);
    assert_eq!(prefs.font_size, 14);

    // Update preferences
    db.update_preference("theme", "light").await.unwrap();
    db.update_preference("editor_mode", "wysiwyg")
        .await
        .unwrap();
    db.update_preference("font_size", "16").await.unwrap();

    // Verify updates
    let updated = db.get_preferences().await.unwrap();
    assert_eq!(updated.theme, "light");
    assert_eq!(updated.editor_mode, EditorMode::FullyRendered);
    assert_eq!(updated.font_size, 16);
}

#[tokio::test]
async fn test_preferences_window_layout() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    let layout = serde_json::json!({
        "left": {"width": 250, "visible": true},
        "right": {"width": 300, "visible": false},
        "activePane": "editor"
    });

    // Update window layout
    db.update_preference("window_layout", &layout.to_string())
        .await
        .unwrap();

    // Retrieve and verify
    let prefs = db.get_preferences().await.unwrap();
    assert!(prefs.window_layout.is_some());

    let retrieved_layout_str = prefs.window_layout.unwrap();
    let retrieved_layout: serde_json::Value = serde_json::from_str(&retrieved_layout_str).unwrap();
    assert_eq!(retrieved_layout["left"]["width"], 250);
    assert_eq!(retrieved_layout["right"]["visible"], false);
}

#[tokio::test]
async fn test_preferences_reset() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    // Modify preferences
    db.update_preference("theme", "light").await.unwrap();
    db.update_preference("font_size", "20").await.unwrap();

    let modified = db.get_preferences().await.unwrap();
    assert_eq!(modified.theme, "light");
    assert_eq!(modified.font_size, 20);

    // Reset to defaults
    db.reset_preferences().await.unwrap();

    let reset = db.get_preferences().await.unwrap();
    assert_eq!(reset.theme, "dark");
    assert_eq!(reset.font_size, 14);
    assert_eq!(reset.editor_mode, EditorMode::SideBySide);
}

#[tokio::test]
async fn test_database_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = format!("sqlite://{}", db_path.display());

    // Create database and add data
    {
        let db = Database::new(&db_path_str).await.unwrap();

        let vault_dir = TempDir::new().unwrap();
        db.create_vault(
            "Persistent Vault".to_string(),
            vault_dir.path().to_str().unwrap().to_string(),
        )
        .await
        .unwrap();

        db.update_preference("theme", "light").await.unwrap();
    }

    // Reconnect to database
    {
        let db = Database::new(&db_path_str).await.unwrap();

        // Verify vault persisted
        let vaults = db.list_vaults().await.unwrap();
        assert_eq!(vaults.len(), 1);
        assert_eq!(vaults[0].name, "Persistent Vault");

        // Verify preferences persisted
        let prefs = db.get_preferences().await.unwrap();
        assert_eq!(prefs.theme, "light");
    }
}

#[tokio::test]
async fn test_concurrent_database_operations() {
    use tokio::task;

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = std::sync::Arc::new(
        Database::new(&format!("sqlite://{}", db_path.display()))
            .await
            .unwrap(),
    );

    let vault_dir = TempDir::new().unwrap();
    let vault = db
        .create_vault(
            "Test Vault".to_string(),
            vault_dir.path().to_str().unwrap().to_string(),
        )
        .await
        .unwrap();

    let vault_id = std::sync::Arc::new(vault.id.clone());

    // Spawn concurrent recent file operations
    let mut handles = vec![];
    for i in 1..=10 {
        let db_clone = db.clone();
        let vault_id_clone = vault_id.clone();

        let handle = task::spawn(async move {
            db_clone
                .record_recent_file(&vault_id_clone, &format!("file{}.md", i))
                .await
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Verify all files were recorded
    let recent = db.get_recent_files(&vault.id, 15).await.unwrap();
    assert_eq!(recent.len(), 10);
}

#[tokio::test]
async fn test_database_transaction_rollback() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    // Attempt to create vault with invalid path
    let result = db
        .create_vault("Invalid Vault".to_string(), "".to_string())
        .await;
    assert!(result.is_err());

    // Verify no vault was created
    let vaults = db.list_vaults().await.unwrap();
    assert_eq!(vaults.len(), 0);
}

#[tokio::test]
async fn test_vault_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    // Create two vaults
    let vault1_dir = TempDir::new().unwrap();
    let vault2_dir = TempDir::new().unwrap();

    let vault1 = db
        .create_vault(
            "Vault 1".to_string(),
            vault1_dir.path().to_str().unwrap().to_string(),
        )
        .await
        .unwrap();

    let vault2 = db
        .create_vault(
            "Vault 2".to_string(),
            vault2_dir.path().to_str().unwrap().to_string(),
        )
        .await
        .unwrap();

    // Add recent files to each vault
    db.record_recent_file(&vault1.id, "vault1_file.md")
        .await
        .unwrap();
    db.record_recent_file(&vault2.id, "vault2_file.md")
        .await
        .unwrap();

    // Verify isolation
    let vault1_recent = db.get_recent_files(&vault1.id, 10).await.unwrap();
    assert_eq!(vault1_recent.len(), 1);
    assert_eq!(vault1_recent[0], "vault1_file.md");

    let vault2_recent = db.get_recent_files(&vault2.id, 10).await.unwrap();
    assert_eq!(vault2_recent.len(), 1);
    assert_eq!(vault2_recent[0], "vault2_file.md");
}
