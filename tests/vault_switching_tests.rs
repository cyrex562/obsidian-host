use obsidian_host::db::Database;
use obsidian_host::services::{FileService, SearchIndex};
use tempfile::TempDir;

#[tokio::test]
async fn test_vault_switching_basic() {
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();
    let vault1_path = temp_dir1.path().to_str().unwrap();
    let vault2_path = temp_dir2.path().to_str().unwrap();

    // Create database
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    // Create two vaults
    let vault1 = db
        .create_vault("Vault 1".to_string(), vault1_path.to_string())
        .await
        .unwrap();
    let vault2 = db
        .create_vault("Vault 2".to_string(), vault2_path.to_string())
        .await
        .unwrap();

    // Create files in each vault
    FileService::create_file(vault1_path, "vault1_note.md", Some("# Vault 1 Note")).unwrap();
    FileService::create_file(vault2_path, "vault2_note.md", Some("# Vault 2 Note")).unwrap();

    // Verify files are isolated
    let vault1_file = FileService::read_file(vault1_path, "vault1_note.md").unwrap();
    assert_eq!(vault1_file.content, "# Vault 1 Note");

    let vault2_file = FileService::read_file(vault2_path, "vault2_note.md").unwrap();
    assert_eq!(vault2_file.content, "# Vault 2 Note");

    // Verify cross-vault access fails
    let cross_access = FileService::read_file(vault1_path, "vault2_note.md");
    assert!(cross_access.is_err());
}

#[tokio::test]
async fn test_vault_switching_with_search_index() {
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();
    let vault1_path = temp_dir1.path().to_str().unwrap();
    let vault2_path = temp_dir2.path().to_str().unwrap();

    let search_index = SearchIndex::new();

    // Create files in vault 1
    FileService::create_file(vault1_path, "note1.md", Some("# Vault 1\n\nRust content")).unwrap();
    search_index
        .update_file(
            "vault1",
            "note1.md",
            "# Vault 1\n\nRust content".to_string(),
        )
        .unwrap();

    // Create files in vault 2
    FileService::create_file(vault2_path, "note2.md", Some("# Vault 2\n\nPython content")).unwrap();
    search_index
        .update_file(
            "vault2",
            "note2.md",
            "# Vault 2\n\nPython content".to_string(),
        )
        .unwrap();

    // Search in vault 1
    let results1 = search_index.search("vault1", "Rust", 1, 10).unwrap();
    assert_eq!(results1.results.len(), 1);
    assert_eq!(results1.results[0].path, "note1.md");

    // Search in vault 2
    let results2 = search_index.search("vault2", "Python", 1, 10).unwrap();
    assert_eq!(results2.results.len(), 1);
    assert_eq!(results2.results[0].path, "note2.md");

    // Verify isolation - vault 1 shouldn't find Python content
    let cross_search1 = search_index.search("vault1", "Python", 1, 10).unwrap();
    assert_eq!(cross_search1.results.len(), 0);

    // Verify isolation - vault 2 shouldn't find Rust content
    let cross_search2 = search_index.search("vault2", "Rust", 1, 10).unwrap();
    assert_eq!(cross_search2.results.len(), 0);
}

#[tokio::test]
async fn test_vault_switching_with_recent_files() {
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();
    let vault1_path = temp_dir1.path().to_str().unwrap();
    let vault2_path = temp_dir2.path().to_str().unwrap();

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    let vault1 = db
        .create_vault("Vault 1".to_string(), vault1_path.to_string())
        .await
        .unwrap();
    let vault2 = db
        .create_vault("Vault 2".to_string(), vault2_path.to_string())
        .await
        .unwrap();

    // Record recent files in vault 1
    db.record_recent_file(&vault1.id, "file1.md").await.unwrap();
    db.record_recent_file(&vault1.id, "file2.md").await.unwrap();

    // Record recent files in vault 2
    db.record_recent_file(&vault2.id, "file3.md").await.unwrap();
    db.record_recent_file(&vault2.id, "file4.md").await.unwrap();

    // Get recent files for vault 1
    let recent1 = db.get_recent_files(&vault1.id, 10).await.unwrap();
    assert_eq!(recent1.len(), 2);
    assert!(recent1.contains(&"file1.md".to_string()));
    assert!(recent1.contains(&"file2.md".to_string()));

    // Get recent files for vault 2
    let recent2 = db.get_recent_files(&vault2.id, 10).await.unwrap();
    assert_eq!(recent2.len(), 2);
    assert!(recent2.contains(&"file3.md".to_string()));
    assert!(recent2.contains(&"file4.md".to_string()));

    // Verify isolation
    assert!(!recent1.contains(&"file3.md".to_string()));
    assert!(!recent2.contains(&"file1.md".to_string()));
}

#[tokio::test]
async fn test_vault_deletion_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    let vault = db
        .create_vault("Test Vault".to_string(), vault_path.to_string())
        .await
        .unwrap();

    // Record some recent files
    db.record_recent_file(&vault.id, "file1.md").await.unwrap();
    db.record_recent_file(&vault.id, "file2.md").await.unwrap();

    // Verify recent files exist
    let recent_before = db.get_recent_files(&vault.id, 10).await.unwrap();
    assert_eq!(recent_before.len(), 2);

    // Delete vault
    db.delete_vault(&vault.id).await.unwrap();

    // Verify vault is deleted
    let vault_result = db.get_vault(&vault.id).await;
    assert!(vault_result.is_err());

    // Recent files should also be cleaned up (cascade delete)
    let recent_after = db.get_recent_files(&vault.id, 10).await.unwrap();
    assert_eq!(recent_after.len(), 0);
}

#[tokio::test]
async fn test_multiple_vault_operations() {
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    // Create multiple vaults
    let mut vaults = Vec::new();
    for i in 1..=5 {
        let temp_dir = TempDir::new().unwrap();
        let vault_path = temp_dir.path().to_str().unwrap();
        let vault = db
            .create_vault(format!("Vault {}", i), vault_path.to_string())
            .await
            .unwrap();
        vaults.push((vault, temp_dir));
    }

    // List all vaults
    let all_vaults = db.list_vaults().await.unwrap();
    assert_eq!(all_vaults.len(), 5);

    // Verify each vault
    for (vault, _) in &vaults {
        let retrieved = db.get_vault(&vault.id).await.unwrap();
        assert_eq!(retrieved.id, vault.id);
        assert_eq!(retrieved.name, vault.name);
    }

    // Delete some vaults
    db.delete_vault(&vaults[0].0.id).await.unwrap();
    db.delete_vault(&vaults[2].0.id).await.unwrap();

    // Verify count
    let remaining_vaults = db.list_vaults().await.unwrap();
    assert_eq!(remaining_vaults.len(), 3);
}

#[tokio::test]
async fn test_vault_switching_preserves_state() {
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();
    let vault1_path = temp_dir1.path().to_str().unwrap();
    let vault2_path = temp_dir2.path().to_str().unwrap();

    let search_index = SearchIndex::new();

    // Setup vault 1
    FileService::create_file(vault1_path, "note1.md", Some("Content 1")).unwrap();
    search_index
        .update_file("vault1", "note1.md", "Content 1".to_string())
        .unwrap();

    // Setup vault 2
    FileService::create_file(vault2_path, "note2.md", Some("Content 2")).unwrap();
    search_index
        .update_file("vault2", "note2.md", "Content 2".to_string())
        .unwrap();

    // "Switch" to vault 1 and verify
    let results1 = search_index.search("vault1", "Content", 1, 10).unwrap();
    assert_eq!(results1.results.len(), 1);
    assert_eq!(results1.results[0].path, "note1.md");

    // "Switch" to vault 2 and verify
    let results2 = search_index.search("vault2", "Content", 1, 10).unwrap();
    assert_eq!(results2.results.len(), 1);
    assert_eq!(results2.results[0].path, "note2.md");

    // "Switch" back to vault 1 - state should be preserved
    let results1_again = search_index.search("vault1", "Content", 1, 10).unwrap();
    assert_eq!(results1_again.results.len(), 1);
    assert_eq!(results1_again.results[0].path, "note1.md");
}
