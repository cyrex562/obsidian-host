use obsidian_host::services::MarkdownService;

#[test]
fn test_basic_embed() {
    let markdown = "Here is an image: ![[image.png]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should render as an image tag
    assert!(html.contains("<img"));
    assert!(html.contains("src=\"image.png\""));
    assert!(html.contains("alt=\"image.png\""));
}

#[test]
fn test_embed_with_alt_text() {
    let markdown = "![[image.png|Alt Text]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should render as an image tag with alt text
    assert!(html.contains("<img"));
    assert!(html.contains("src=\"image.png\""));
    assert!(html.contains("alt=\"Alt Text\""));
}

#[test]
fn test_embed_and_link_mixed() {
    let markdown = "Link: [[note]] and Embed: ![[image.png]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should verify both exist correctly
    assert!(html.contains("<a href=\"note\">note</a>"));
    assert!(html.contains("<img"));
    assert!(html.contains("src=\"image.png\""));
}

#[test]
fn test_embed_special_chars() {
    let markdown = "![[image with spaces.png]]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should be URL encoded
    assert!(html.contains("src=\"image%20with%20spaces.png\""));
}
