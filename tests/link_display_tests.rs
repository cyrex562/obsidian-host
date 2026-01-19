use obsidian_host::services::MarkdownService;

#[test]
fn test_header_link_display_text() {
    let markdown = "Check [[Note#Header]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should display "Note > Header"
    // Regex for > might need escaping in HTML check if it becomes &gt;
    // <a href="Note#Header">Note &gt; Header</a>
    assert!(html.contains("Note > Header") || html.contains("Note &gt; Header"));
}

#[test]
fn test_block_ref_display_text() {
    let markdown = "Check [[Note#^block]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should display "Note > ^block"
    assert!(html.contains("Note > ^block") || html.contains("Note &gt; ^block"));
}

#[test]
fn test_header_link_with_alias_display() {
    let markdown = "Check [[Note#Header|Custom Alias]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should display "Custom Alias" ignoring the heuristic
    assert!(html.contains(">Custom Alias</a>"));
    assert!(!html.contains("Note > Header"));
}
