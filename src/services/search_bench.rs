#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::time::Instant;
    use tempfile::TempDir;

    fn create_large_vault(files: usize) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let vault = temp_dir.path();
        
        for i in 0..files {
            let content = format!("This is file number {} with some common words like rust and search.\nRepeating content to make it longer.\n", i);
            fs::write(vault.join(format!("note_{}.md", i)), content).unwrap();
        }
        temp_dir
    }

    #[test]
    #[ignore]
    fn benchmark_search() {
        let size = 1000;
        let temp = create_large_vault(size);
        let vault_path = temp.path().to_str().unwrap();
        let index = SearchIndex::new();
        
        let start_index = Instant::now();
        index.index_vault("bench", vault_path).unwrap();
        println!("Indexed {} files in {:?}", size, start_index.elapsed());

        let start_search = Instant::now();
        let _ = index.search("bench", "rust", 1, 10);
        println!("Search took {:?}", start_search.elapsed());
    }
}
