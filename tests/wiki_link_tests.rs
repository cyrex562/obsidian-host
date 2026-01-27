use obsidian_host::services::MarkdownService;

#[test]
fn test_basic_wiki_link() {
    let markdown = "Check out [[Note]] for more info.";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should contain a link to "Note"
    assert!(html.contains("<a href=\"Note\""));
    assert!(html.contains(">Note</a>"));
}

#[test]
fn test_wiki_link_with_alias() {
    let markdown = "Check out [[Note|this link]] for more info.";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should contain a link to "Note" with text "this link"
    assert!(html.contains("<a href=\"Note\""));
    assert!(html.contains(">this link</a>"));
}

#[test]
fn test_multiple_wiki_links() {
    let markdown = "[[Note 1]] and [[Note 2|Alias 2]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("href=\"Note%201\""));
    assert!(html.contains(">Note 1</a>"));
    assert!(html.contains("href=\"Note%202\""));
    assert!(html.contains(">Alias 2</a>"));
}

#[test]
fn test_wiki_link_inside_formatting() {
    let markdown = "**[[Note]]**";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<strong>"));
    assert!(html.contains("href=\"Note\""));
    assert!(html.contains(">Note</a>"));
    assert!(html.contains("</strong>"));
}

#[test]
fn test_wiki_link_not_in_code_block() {
    let markdown = "`[[Note]]`";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should NOT be a link
    assert!(!html.contains("<a href"));
    assert!(html.contains("<code>[[Note]]</code>") || html.contains("<code>[[Note]]</code>"));
}

#[test]
fn test_wiki_link_with_special_chars() {
    let markdown = "[[Note with spaces]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("href=\"Note%20with%20spaces\""));
    assert!(html.contains(">Note with spaces</a>"));
}
