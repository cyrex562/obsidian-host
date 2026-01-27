use obsidian_host::db::Database;
use obsidian_host::services::{FileService, SearchIndex};
use std::fs;
use tempfile::TempDir;

/// Test full file CRUD (Create, Read, Update, Delete) workflow
#[tokio::test]
async fn test_file_crud_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // CREATE: Create a new file
    let file_path = "test_note.md";
    let initial_content = "# Test Note\n\nThis is the initial content.";

    let created = FileService::create_file(vault_path, file_path, Some(initial_content)).unwrap();
    assert_eq!(created.path, file_path);
    assert_eq!(created.content, initial_content);

    // Verify file exists on disk
    let full_path = temp_dir.path().join(file_path);
    assert!(full_path.exists());

    // READ: Read the file back
    let read_file = FileService::read_file(vault_path, file_path).unwrap();
    assert_eq!(read_file.content, initial_content);
    assert_eq!(read_file.path, file_path);

    // UPDATE: Modify the file
    let updated_content = "# Test Note\n\nThis is the updated content.\n\n## New Section";
    let updated = FileService::write_file(
        vault_path,
        file_path,
        updated_content,
        Some(read_file.modified),
        None,
    )
    .unwrap();

    assert_eq!(updated.content, updated_content);

    // Read again to verify update
    let read_updated = FileService::read_file(vault_path, file_path).unwrap();
    assert_eq!(read_updated.content, updated_content);

    // DELETE: Delete the file
    FileService::delete_file(vault_path, file_path).unwrap();

    // Verify file is moved to trash
    let trash_dir = temp_dir.path().join(".trash");
    assert!(trash_dir.exists());

    // Verify original file is gone
    assert!(!full_path.exists());
}

#[tokio::test]
async fn test_file_crud_with_subdirectories() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // Create file in subdirectory
    let file_path = "folder/subfolder/nested_note.md";
    let content = "# Nested Note";

    let created = FileService::create_file(vault_path, file_path, Some(content)).unwrap();
    assert_eq!(created.path, file_path);

    // Verify directory structure was created
    let full_path = temp_dir.path().join(file_path);
    assert!(full_path.exists());
    assert!(full_path.parent().unwrap().exists());

    // Read the file
    let read = FileService::read_file(vault_path, file_path).unwrap();
    assert_eq!(read.content, content);

    // Update the file
    let new_content = "# Nested Note\n\nUpdated content";
    FileService::write_file(
        vault_path,
        file_path,
        new_content,
        Some(read.modified),
        None,
    )
    .unwrap();

    // Delete the file
    FileService::delete_file(vault_path, file_path).unwrap();
    assert!(!full_path.exists());
}

#[tokio::test]
async fn test_file_crud_with_frontmatter() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    let file_path = "note_with_frontmatter.md";
    let content = "# Note with Frontmatter\n\nContent here.";
    let frontmatter = serde_json::json!({
        "title": "My Note",
        "tags": ["test", "integration"],
        "created": "2026-01-23"
    });

    // Create with frontmatter
    FileService::create_file(vault_path, file_path, Some(content)).unwrap();
    let updated =
        FileService::write_file(vault_path, file_path, content, None, Some(&frontmatter)).unwrap();

    // Read and verify frontmatter
    let read = FileService::read_file(vault_path, file_path).unwrap();
    assert!(read.frontmatter.is_some());
    let fm = read.frontmatter.unwrap();
    assert_eq!(fm["title"], "My Note");
    assert_eq!(fm["tags"][0], "test");

    // Update frontmatter
    let new_frontmatter = serde_json::json!({
        "title": "Updated Title",
        "tags": ["test", "integration", "updated"],
        "modified": "2026-01-23"
    });

    FileService::write_file(
        vault_path,
        file_path,
        content,
        Some(read.modified),
        Some(&new_frontmatter),
    )
    .unwrap();

    // Verify update
    let read_updated = FileService::read_file(vault_path, file_path).unwrap();
    let fm_updated = read_updated.frontmatter.unwrap();
    assert_eq!(fm_updated["title"], "Updated Title");
    assert_eq!(fm_updated["tags"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_file_crud_with_search_index() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();
    let vault_id = "test-vault";

    let search_index = SearchIndex::new();

    // Create file
    let file_path = "searchable_note.md";
    let content =
        "# Searchable Note\n\nThis note contains searchable content about Rust programming.";

    FileService::create_file(vault_path, file_path, Some(content)).unwrap();

    // Index the file
    search_index
        .update_file(vault_id, file_path, content.to_string())
        .unwrap();

    // Search for content
    let results = search_index
        .search(vault_id, "Rust programming", 1, 10)
        .unwrap();
    assert_eq!(results.results.len(), 1);
    assert_eq!(results.results[0].path, file_path);

    // Update file
    let new_content = "# Searchable Note\n\nUpdated content about Python programming.";
    FileService::write_file(vault_path, file_path, new_content, None, None).unwrap();

    // Update index
    search_index
        .update_file(vault_id, file_path, new_content.to_string())
        .unwrap();

    // Search for new content
    let results_python = search_index.search(vault_id, "Python", 1, 10).unwrap();
    assert_eq!(results_python.results.len(), 1);

    // Old content should not be found
    let results_rust = search_index.search(vault_id, "Rust", 1, 10).unwrap();
    assert_eq!(results_rust.results.len(), 0);

    // Delete file
    FileService::delete_file(vault_path, file_path).unwrap();

    // Remove from index
    search_index.remove_file(vault_id, file_path).unwrap();

    // Verify not in search results
    let results_after_delete = search_index.search(vault_id, "Python", 1, 10).unwrap();
    assert_eq!(results_after_delete.results.len(), 0);
}

#[tokio::test]
async fn test_multiple_file_operations() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // Create multiple files
    let files = vec![
        ("note1.md", "# Note 1"),
        ("note2.md", "# Note 2"),
        ("folder/note3.md", "# Note 3"),
    ];

    for (path, content) in &files {
        FileService::create_file(vault_path, path, Some(content)).unwrap();
    }

    // Verify all files exist
    for (path, _) in &files {
        let read = FileService::read_file(vault_path, path).unwrap();
        assert_eq!(read.path, *path);
    }

    // Get file tree
    let tree = FileService::get_file_tree(vault_path).unwrap();
    assert!(!tree.is_empty());

    // Update all files
    for (path, _) in &files {
        let read = FileService::read_file(vault_path, path).unwrap();
        let new_content = format!("{}\n\nUpdated content", read.content);
        FileService::write_file(vault_path, path, &new_content, Some(read.modified), None).unwrap();
    }

    // Delete all files
    for (path, _) in &files {
        FileService::delete_file(vault_path, path).unwrap();
    }

    // Verify trash directory exists
    let trash_dir = temp_dir.path().join(".trash");
    assert!(trash_dir.exists());
}

#[tokio::test]
async fn test_file_operations_with_database() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // Create database
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();

    // Create vault
    let vault = db
        .create_vault("Test Vault".to_string(), vault_path.to_string())
        .await
        .unwrap();

    // Create file
    let file_path = "database_test.md";
    let content = "# Database Test";
    FileService::create_file(vault_path, file_path, Some(content)).unwrap();

    // Record as recent file
    db.record_recent_file(&vault.id, file_path).await.unwrap();

    // Get recent files
    let recent = db.get_recent_files(&vault.id, 10).await.unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0], file_path);

    // Update file
    let new_content = "# Database Test\n\nUpdated";
    FileService::write_file(vault_path, file_path, new_content, None, None).unwrap();

    // Record again (should update timestamp)
    db.record_recent_file(&vault.id, file_path).await.unwrap();

    // Still only one entry
    let recent_after = db.get_recent_files(&vault.id, 10).await.unwrap();
    assert_eq!(recent_after.len(), 1);

    // Delete file
    FileService::delete_file(vault_path, file_path).unwrap();

    // Recent files entry still exists (not automatically cleaned up)
    let recent_final = db.get_recent_files(&vault.id, 10).await.unwrap();
    assert_eq!(recent_final.len(), 1);
}
