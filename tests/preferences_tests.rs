use obsidian_host::db::Database;
use obsidian_host::models::{EditorMode, UserPreferences};
use tempfile::TempDir;

async fn create_test_db() -> (Database, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();
    (db, temp_dir)
}

#[tokio::test]
async fn test_default_preferences() {
    let (db, _temp) = create_test_db().await;

    let prefs = db.get_preferences().await.unwrap();

    assert_eq!(prefs.theme, "dark");
    assert!(matches!(prefs.editor_mode, EditorMode::SideBySide));
    assert_eq!(prefs.font_size, 14);
    assert!(prefs.window_layout.is_none());
}

#[tokio::test]
async fn test_update_preferences() {
    let (db, _temp) = create_test_db().await;

    let new_prefs = UserPreferences {
        theme: "light".to_string(),
        editor_mode: EditorMode::Raw,
        font_size: 16,
        window_layout: Some(r#"{"panes":[{"id":"pane-1"}]}"#.to_string()),
    };

    db.update_preferences(&new_prefs).await.unwrap();

    let retrieved = db.get_preferences().await.unwrap();
    assert_eq!(retrieved.theme, "light");
    assert!(matches!(retrieved.editor_mode, EditorMode::Raw));
    assert_eq!(retrieved.font_size, 16);
    assert_eq!(
        retrieved.window_layout,
        Some(r#"{"panes":[{"id":"pane-1"}]}"#.to_string())
    );
}

#[tokio::test]
async fn test_preferences_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}", db_path.display());

    // Create DB and set preferences
    {
        let db = Database::new(&db_url).await.unwrap();
        let prefs = UserPreferences {
            theme: "light".to_string(),
            editor_mode: EditorMode::FullyRendered,
            font_size: 18,
            window_layout: Some("test_layout".to_string()),
        };
        db.update_preferences(&prefs).await.unwrap();
    }

    // Reconnect and verify persistence
    {
        let db = Database::new(&db_url).await.unwrap();
        let prefs = db.get_preferences().await.unwrap();
        assert_eq!(prefs.theme, "light");
        assert!(matches!(prefs.editor_mode, EditorMode::FullyRendered));
        assert_eq!(prefs.font_size, 18);
        assert_eq!(prefs.window_layout, Some("test_layout".to_string()));
    }
}

#[tokio::test]
async fn test_all_editor_modes() {
    let (db, _temp) = create_test_db().await;

    let modes = vec![
        EditorMode::Raw,
        EditorMode::SideBySide,
        EditorMode::FormattedRaw,
        EditorMode::FullyRendered,
    ];

    for mode in modes {
        let prefs = UserPreferences {
            theme: "dark".to_string(),
            editor_mode: mode.clone(),
            font_size: 14,
            window_layout: None,
        };

        db.update_preferences(&prefs).await.unwrap();
        let retrieved = db.get_preferences().await.unwrap();

        match mode {
            EditorMode::Raw => assert!(matches!(retrieved.editor_mode, EditorMode::Raw)),
            EditorMode::SideBySide => {
                assert!(matches!(retrieved.editor_mode, EditorMode::SideBySide))
            }
            EditorMode::FormattedRaw => {
                assert!(matches!(retrieved.editor_mode, EditorMode::FormattedRaw))
            }
            EditorMode::FullyRendered => {
                assert!(matches!(retrieved.editor_mode, EditorMode::FullyRendered))
            }
        }
    }
}

#[tokio::test]
async fn test_recent_files() {
    let (db, _temp) = create_test_db().await;

    // Create a test vault
    let vault = db
        .create_vault("Test Vault".to_string(), "/tmp/test".to_string())
        .await
        .unwrap();

    // Record some recent files
    db.record_recent_file(&vault.id, "file1.md").await.unwrap();
    db.record_recent_file(&vault.id, "file2.md").await.unwrap();
    db.record_recent_file(&vault.id, "file3.md").await.unwrap();

    // Get recent files
    let recent = db.get_recent_files(&vault.id, 10).await.unwrap();

    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0], "file3.md"); // Most recent first
    assert_eq!(recent[1], "file2.md");
    assert_eq!(recent[2], "file1.md");
}

#[tokio::test]
async fn test_recent_files_limit() {
    let (db, _temp) = create_test_db().await;

    let vault = db
        .create_vault("Test Vault".to_string(), "/tmp/test".to_string())
        .await
        .unwrap();

    // Record 10 files
    for i in 1..=10 {
        db.record_recent_file(&vault.id, &format!("file{}.md", i))
            .await
            .unwrap();
    }

    // Request only 5
    let recent = db.get_recent_files(&vault.id, 5).await.unwrap();
    assert_eq!(recent.len(), 5);
    assert_eq!(recent[0], "file10.md"); // Most recent
}

#[tokio::test]
async fn test_recent_files_update_timestamp() {
    let (db, _temp) = create_test_db().await;

    let vault = db
        .create_vault("Test Vault".to_string(), "/tmp/test".to_string())
        .await
        .unwrap();

    // Record file1, then file2, then file1 again
    db.record_recent_file(&vault.id, "file1.md").await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    db.record_recent_file(&vault.id, "file2.md").await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    db.record_recent_file(&vault.id, "file1.md").await.unwrap();

    let recent = db.get_recent_files(&vault.id, 10).await.unwrap();

    // file1 should be first (most recent) even though it was accessed first originally
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0], "file1.md");
    assert_eq!(recent[1], "file2.md");
}

#[tokio::test]
async fn test_recent_files_per_vault() {
    let (db, _temp) = create_test_db().await;

    let vault1 = db
        .create_vault("Vault 1".to_string(), "/tmp/vault1".to_string())
        .await
        .unwrap();
    let vault2 = db
        .create_vault("Vault 2".to_string(), "/tmp/vault2".to_string())
        .await
        .unwrap();

    // Record files in different vaults
    db.record_recent_file(&vault1.id, "vault1_file.md")
        .await
        .unwrap();
    db.record_recent_file(&vault2.id, "vault2_file.md")
        .await
        .unwrap();

    let recent1 = db.get_recent_files(&vault1.id, 10).await.unwrap();
    let recent2 = db.get_recent_files(&vault2.id, 10).await.unwrap();

    assert_eq!(recent1.len(), 1);
    assert_eq!(recent1[0], "vault1_file.md");

    assert_eq!(recent2.len(), 1);
    assert_eq!(recent2[0], "vault2_file.md");
}

#[tokio::test]
async fn test_window_layout_json() {
    let (db, _temp) = create_test_db().await;

    let complex_layout = r#"{
        "panes": [
            {"id": "pane-1", "activeTabId": "tab-1"},
            {"id": "pane-2", "activeTabId": "tab-2"}
        ],
        "splitOrientation": "vertical",
        "activePaneId": "pane-1"
    }"#;

    let prefs = UserPreferences {
        theme: "dark".to_string(),
        editor_mode: EditorMode::SideBySide,
        font_size: 14,
        window_layout: Some(complex_layout.to_string()),
    };

    db.update_preferences(&prefs).await.unwrap();

    let retrieved = db.get_preferences().await.unwrap();
    assert_eq!(retrieved.window_layout, Some(complex_layout.to_string()));

    // Verify it's valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&retrieved.window_layout.unwrap()).unwrap();
    assert_eq!(parsed["splitOrientation"], "vertical");
}

#[tokio::test]
async fn test_preferences_migration_window_layout() {
    // This test verifies that the window_layout column exists and works
    // even if it was added via migration (ALTER TABLE)
    let (db, _temp) = create_test_db().await;

    // The migration should have run in create_test_db
    // Verify we can use window_layout
    let prefs = UserPreferences {
        theme: "dark".to_string(),
        editor_mode: EditorMode::SideBySide,
        font_size: 14,
        window_layout: Some("test".to_string()),
    };

    db.update_preferences(&prefs).await.unwrap();
    let retrieved = db.get_preferences().await.unwrap();
    assert_eq!(retrieved.window_layout, Some("test".to_string()));
}
