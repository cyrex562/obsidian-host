use obsidian_host::services::MarkdownService;

#[test]
fn test_block_reference() {
    let markdown = "Link to [[Note#^block-id]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Expect: href="Note#^block-id" (or encoded variants of caret?)
    // Browsers/HTML usually keep # unencoded.
    // If it encodes # to %23, it's BROKEN.
    assert!(html.contains("href=\"Note#^block-id\""));
}

#[test]
fn test_header_link() {
    let markdown = "Link to [[Note#Section]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("href=\"Note#Section\""));
}

#[test]
fn test_header_link_with_spaces() {
    let markdown = "Link to [[Note Name#Section Name]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Spaces should be encoded, # should NOT be encoded
    // Note%20Name#Section%20Name
    assert!(html.contains("href=\"Note%20Name#Section%20Name\""));
}

#[test]
fn test_block_ref_with_alias() {
    let markdown = "[[Note#^block|The Block]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("href=\"Note#^block\""));
    assert!(html.contains(">The Block</a>"));
}
