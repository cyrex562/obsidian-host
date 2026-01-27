use obsidian_host::services::{FileService, SearchIndex};
use std::fs;
use std::io::Write;
use std::time::Instant;
use tempfile::TempDir;

#[test]
fn test_large_vault_performance() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();

    // Setup: Create 10,000 files
    println!("Generating 10,000 files...");
    let start_setup = Instant::now();
    for i in 0..10_000 {
        let file_path = temp_dir.path().join(format!("file_{}.md", i));
        // Use minimal content to keep setup time reasonable for this specific test which targets tree walking
        fs::write(file_path, format!("# File {}\nContent for file {}", i, i)).unwrap();
    }
    println!("Setup time: {:.2?}", start_setup.elapsed());

    // Test: Get file tree
    let start_tree = Instant::now();
    let tree = FileService::get_file_tree(vault_path).unwrap();
    let duration = start_tree.elapsed();

    println!("File tree generation time (10k files): {:.2?}", duration);

    // Assert performance (should be under 500ms for 10k files on a reasonable machine)
    // Relaxed slightly to account for CI/slower environments if needed, but 500ms is a good target for simple WalkDir
    assert!(
        duration.as_millis() < 800,
        "File tree generation took too long: {:.2?}",
        duration
    );

    assert_eq!(tree.len(), 10_000);
}

#[test]
fn test_search_performance() {
    let temp_dir = TempDir::new().unwrap();
    // Setup: Create 1,000 files with some searchable content
    let search_index = SearchIndex::new();
    let vault_id = "bench_vault";

    println!("Generating and indexing 1,000 files...");
    let start_setup = Instant::now();
    for i in 0..1000 {
        let content = if i % 100 == 0 {
            format!(
                "# Special Note {}\nThis is a special note with keyword targetdata located here.",
                i
            )
        } else {
            format!(
                "# Note {}\nJust some regular content for note number {}.",
                i, i
            )
        };

        search_index
            .update_file(vault_id, &format!("note_{}.md", i), content)
            .unwrap();
    }
    println!("Indexing time: {:.2?}", start_setup.elapsed());

    // Test: Search performance
    let start_search = Instant::now();
    let results = search_index.search(vault_id, "targetdata", 1, 50).unwrap();
    let duration = start_search.elapsed();

    println!("Search time (1k indexed docs): {:.2?}", duration);

    assert_eq!(results.total_count, 10); // 1000 / 100 = 10 matches
    assert!(
        duration.as_millis() < 50,
        "Search took too long: {:.2?}",
        duration
    );
}

#[test]
fn test_large_file_performance() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().to_str().unwrap();
    let file_path = "large_file.md";

    // generate 10MB of content
    println!("Generating 10MB file...");
    let mut content = String::with_capacity(10 * 1024 * 1024);
    for _ in 0..100_000 {
        content.push_str("Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n");
    }
    let content_len = content.len();
    println!(
        "Generated content size: {:.2} MB",
        content_len as f64 / 1024.0 / 1024.0
    );

    // Test: Write large file
    let start_write = Instant::now();
    FileService::create_file(vault_path, file_path, Some(&content)).unwrap();
    let write_duration = start_write.elapsed();
    println!("Write large file time: {:.2?}", write_duration);

    assert!(
        write_duration.as_millis() < 1000,
        "Writing 10MB file took too long: {:.2?}",
        write_duration
    );

    // Test: Read large file
    let start_read = Instant::now();
    let read_content = FileService::read_file(vault_path, file_path).unwrap();
    let read_duration = start_read.elapsed();
    println!("Read large file time: {:.2?}", read_duration);

    assert_eq!(read_content.content.len(), content_len);
    assert!(
        read_duration.as_millis() < 500,
        "Reading 10MB file took too long: {:.2?}",
        read_duration
    );
}
