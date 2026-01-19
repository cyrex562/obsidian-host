use obsidian_host::services::MarkdownService;

#[test]
fn test_frontmatter_suppression() {
    let markdown = "---\ntitle: Hello World\ntags: [a, b]\n---\n# Content";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Frontmatter should NOT be rendered
    assert!(!html.contains("title: Hello World"));
    assert!(!html.contains("tags: [a, b]"));
    assert!(html.contains("<h1>Content</h1>"));
}

#[test]
fn test_frontmatter_with_wiki_link_logic() {
    // Ensure our custom event loop doesn't break parsing or accidentally render it
    let markdown = "---\nkey: [[value]]\n---\nText";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(!html.contains("key:"));
    assert!(html.contains("<p>Text</p>"));
}
