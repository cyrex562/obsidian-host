use obsidian_host::services::FileService;
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

#[test]
fn test_large_vault_performance() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // Setup: Create 10,000 files
    let start_setup = Instant::now();
    for i in 0..10_000 {
        let file_path = temp_dir.path().join(format!("file_{}.md", i));
        fs::write(file_path, format!("# File {}\nContent for file {}", i, i)).unwrap();
    }
    println!("Setup time: {:.2?}", start_setup.elapsed());

    // Test: Get file tree
    let start_tree = Instant::now();
    let tree = FileService::get_file_tree(vault_path).unwrap();
    let duration = start_tree.elapsed();

    println!("File tree generation time: {:.2?}", duration);

    // Assert performance (should be under 500ms for 10k files)
    assert!(
        duration.as_millis() < 500,
        "File tree generation took too long: {:.2?}",
        duration
    );

    // basic verification
    assert_eq!(tree.len(), 10_000);
}
