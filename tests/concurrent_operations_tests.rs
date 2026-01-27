use obsidian_host::services::{FileService, SearchIndex};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::task;

#[tokio::test]
async fn test_concurrent_file_reads() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // Create a file
    let content = "# Concurrent Read Test\n\nThis file will be read concurrently.";
    FileService::create_file(vault_path, "concurrent.md", Some(content)).unwrap();

    let vault_path_arc = Arc::new(vault_path.to_string());

    // Spawn multiple concurrent read tasks
    let mut handles = vec![];
    for _ in 0..10 {
        let vault_path_clone = vault_path_arc.clone();
        let handle =
            task::spawn(async move { FileService::read_file(&vault_path_clone, "concurrent.md") });
        handles.push(handle);
    }

    // Wait for all reads to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        let file = result.unwrap();
        assert_eq!(file.content, content);
    }
}

#[tokio::test]
async fn test_concurrent_search_operations() {
    let search_index = Arc::new(SearchIndex::new());
    let vault_id = "test-vault";

    // Index some files
    for i in 1..=10 {
        search_index
            .update_file(
                vault_id,
                &format!("file{}.md", i),
                format!("Content for file {}", i),
            )
            .unwrap();
    }

    // Perform concurrent searches
    let mut handles = vec![];
    for i in 1..=20 {
        let index_clone = search_index.clone();
        let query = if i % 2 == 0 { "Content" } else { "file" };
        let handle = task::spawn(async move { index_clone.search(vault_id, query, 1, 10) });
        handles.push(handle);
    }

    // Verify all searches complete successfully
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        let paged_results = result.unwrap();
        assert!(!paged_results.results.is_empty());
    }
}

#[tokio::test]
async fn test_concurrent_index_updates() {
    let search_index = Arc::new(SearchIndex::new());
    let vault_id = "test-vault";

    // Concurrently update different files
    let mut handles = vec![];
    for i in 1..=20 {
        let index_clone = search_index.clone();
        let handle = task::spawn(async move {
            index_clone.update_file(
                vault_id,
                &format!("concurrent{}.md", i),
                format!("Concurrent content {}", i),
            )
        });
        handles.push(handle);
    }

    // Wait for all updates
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Verify all files are indexed
    let paged_results = search_index.search(vault_id, "Concurrent", 1, 25).unwrap();
    assert_eq!(paged_results.results.len(), 20);
}

#[tokio::test]
async fn test_concurrent_file_tree_access() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // Create multiple files
    for i in 1..=5 {
        FileService::create_file(
            vault_path,
            &format!("file{}.md", i),
            Some(&format!("Content {}", i)),
        )
        .unwrap();
    }

    let vault_path_arc = Arc::new(vault_path.to_string());

    // Concurrently access file tree
    let mut handles = vec![];
    for _ in 0..10 {
        let vault_path_clone = vault_path_arc.clone();
        let handle = task::spawn(async move { FileService::get_file_tree(&vault_path_clone) });
        handles.push(handle);
    }

    // Verify all accesses succeed
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        let tree = result.unwrap();
        assert!(!tree.is_empty());
    }
}

#[tokio::test]
async fn test_concurrent_search_and_update() {
    let search_index = Arc::new(SearchIndex::new());
    let vault_id = "test-vault";

    // Initial indexing
    for i in 1..=10 {
        search_index
            .update_file(
                vault_id,
                &format!("file{}.md", i),
                format!("Initial content {}", i),
            )
            .unwrap();
    }

    let mut search_handles = vec![];
    let mut update_handles = vec![];

    // Spawn search tasks
    for _ in 0..10 {
        let index_clone = search_index.clone();
        let handle = task::spawn(async move { index_clone.search(vault_id, "content", 1, 10) });
        search_handles.push(handle);
    }

    // Spawn update tasks
    for i in 1..=5 {
        let index_clone = search_index.clone();
        let handle = task::spawn(async move {
            index_clone.update_file(
                vault_id,
                &format!("file{}.md", i),
                format!("Updated content {}", i),
            )
        });
        update_handles.push(handle);
    }

    // Wait for all search operations
    for handle in search_handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Wait for all update operations
    for handle in update_handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Verify final state
    let final_results = search_index.search(vault_id, "content", 1, 15).unwrap();
    assert_eq!(final_results.results.len(), 10);
}

#[tokio::test]
async fn test_concurrent_vault_operations() {
    use obsidian_host::db::Database;

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let db = Arc::new(
        Database::new(&format!("sqlite://{}", db_path.display()))
            .await
            .unwrap(),
    );

    // Concurrently create vaults
    let mut handles = vec![];
    for i in 1..=10 {
        let db_clone = db.clone();
        let temp_dir = TempDir::new().unwrap();
        let vault_path = temp_dir.path().to_str().unwrap().to_string();

        let handle = task::spawn(async move {
            db_clone
                .create_vault(format!("Vault {}", i), vault_path)
                .await
        });
        handles.push((handle, temp_dir));
    }

    // Wait for all creations
    for (handle, _temp_dir) in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Verify all vaults were created
    let vaults = db.list_vaults().await.unwrap();
    assert_eq!(vaults.len(), 10);
}

#[tokio::test]
async fn test_concurrent_recent_file_updates() {
    use obsidian_host::db::Database;

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let db = Arc::new(
        Database::new(&format!("sqlite://{}", db_path.display()))
            .await
            .unwrap(),
    );

    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();
    let vault = db
        .create_vault("Test Vault".to_string(), vault_path.to_string())
        .await
        .unwrap();

    let vault_id = Arc::new(vault.id.clone());

    // Concurrently record recent files
    let mut handles = vec![];
    for i in 1..=20 {
        let db_clone = db.clone();
        let vault_id_clone = vault_id.clone();
        let handle = task::spawn(async move {
            db_clone
                .record_recent_file(&vault_id_clone, &format!("file{}.md", i))
                .await
        });
        handles.push(handle);
    }

    // Wait for all records
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Verify all files were recorded
    let recent = db.get_recent_files(&vault.id, 25).await.unwrap();
    assert_eq!(recent.len(), 20);
}

#[tokio::test]
async fn test_concurrent_file_modifications() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // Create initial files
    for i in 1..=10 {
        FileService::create_file(
            vault_path,
            &format!("file{}.md", i),
            Some(&format!("Initial {}", i)),
        )
        .unwrap();
    }

    let vault_path_arc = Arc::new(vault_path.to_string());

    // Concurrently modify different files
    let mut handles = vec![];
    for i in 1..=10 {
        let vault_path_clone = vault_path_arc.clone();
        let handle = task::spawn(async move {
            let file_path = format!("file{}.md", i);
            FileService::write_file(
                &vault_path_clone,
                &file_path,
                &format!("Updated {}", i),
                None,
                None,
            )
        });
        handles.push(handle);
    }

    // Wait for all modifications
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Verify all files were updated
    for i in 1..=10 {
        let file = FileService::read_file(vault_path, &format!("file{}.md", i)).unwrap();
        assert_eq!(file.content, format!("Updated {}", i));
    }
}
