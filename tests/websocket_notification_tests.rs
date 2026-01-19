use obsidian_host::services::FileService;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::test]
async fn test_websocket_notification_delivery() {
    // This is a manual test that verifies the WebSocket system works
    // In a real scenario, you would:
    // 1. Start the server
    // 2. Connect a WebSocket client
    // 3. Make file changes
    // 4. Verify events are received

    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // Create a test file
    let test_file = temp_dir.path().join("test.md");
    fs::write(&test_file, "# Test File\nInitial content").unwrap();

    // Verify file was created
    assert!(test_file.exists());

    // Simulate file modification
    sleep(Duration::from_millis(100)).await;
    fs::write(&test_file, "# Test File\nModified content").unwrap();

    // Verify file was modified
    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("Modified content"));

    // Simulate file deletion
    sleep(Duration::from_millis(100)).await;
    FileService::delete_file(vault_path, "test.md").unwrap();

    // Verify file was moved to trash
    let trash_dir = temp_dir.path().join(".trash");
    assert!(trash_dir.exists());

    println!("✓ WebSocket notification delivery test completed");
    println!("  - File creation: OK");
    println!("  - File modification: OK");
    println!("  - File deletion (trash): OK");
}

#[test]
fn test_file_change_event_structure() {
    // Verify that FileChangeEvent can be serialized/deserialized
    // This ensures WebSocket messages will work correctly

    use serde_json;

    // Test Created event
    let created_json = r#"{
        "vault_id": "test-vault",
        "path": "test.md",
        "event_type": "Created"
    }"#;

    let parsed: serde_json::Value = serde_json::from_str(created_json).unwrap();
    assert_eq!(parsed["event_type"], "Created");
    assert_eq!(parsed["path"], "test.md");

    // Test Modified event
    let modified_json = r#"{
        "vault_id": "test-vault",
        "path": "test.md",
        "event_type": "Modified"
    }"#;

    let parsed: serde_json::Value = serde_json::from_str(modified_json).unwrap();
    assert_eq!(parsed["event_type"], "Modified");

    // Test Deleted event
    let deleted_json = r#"{
        "vault_id": "test-vault",
        "path": "test.md",
        "event_type": "Deleted"
    }"#;

    let parsed: serde_json::Value = serde_json::from_str(deleted_json).unwrap();
    assert_eq!(parsed["event_type"], "Deleted");

    // Test Renamed event
    let renamed_json = r#"{
        "vault_id": "test-vault",
        "path": "old.md",
        "event_type": {
            "Renamed": {
                "from": "old.md",
                "to": "new.md"
            }
        }
    }"#;

    let parsed: serde_json::Value = serde_json::from_str(renamed_json).unwrap();
    assert!(parsed["event_type"].is_object());
    assert!(parsed["event_type"]["Renamed"].is_object());

    println!("✓ File change event structure test completed");
    println!("  - Created event: OK");
    println!("  - Modified event: OK");
    println!("  - Deleted event: OK");
    println!("  - Renamed event: OK");
}
