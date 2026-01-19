use obsidian_host::services::MarkdownService;

#[test]
fn test_basic_tag() {
    let markdown = "This is a #tag example";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<a href=\"#tag\" class=\"tag\">#tag</a>"));
}

#[test]
fn test_tag_after_newline() {
    let markdown = "Line 1\n#tag at start";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // <p>Line 1 #tag at start</p> (if soft break)
    // or <p>Line 1</p><p>#tag at start</p> (if double newline)
    // In this case, single newline is a soft break (space).
    assert!(html.contains("<a href=\"#tag\" class=\"tag\">#tag</a>"));
}

#[test]
fn test_nested_tag() {
    let markdown = "Nested #parent/child tag";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<a href=\"#parent/child\" class=\"tag\">#parent/child</a>"));
}

#[test]
fn test_invalid_tags() {
    let markdown = "Not a tag#1 because no space. Also #123 is numeric, not tag. #tag.with.dots";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // "tag#1" -> text
    assert!(html.contains("tag#1"));

    // "#123" -> text
    assert!(html.contains("#123"));
    assert!(!html.contains("<a href=\"#123\""));

    // "#tag.with.dots" -> regex only allows _ - / ?
    // My regex was `[a-zA-Z0-9_\-/]+`. So it stops at dot.
    // So #tag is a tag, .with.dots is text after?
    // " #tag.with.dots" -> <a..>#tag</a>.with.dots
    assert!(html.contains("<a href=\"#tag\" class=\"tag\">#tag</a>.with.dots"));
}

#[test]
fn test_tag_and_wiki_link() {
    let markdown = "Check [[Link]] and #tag";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<a href=\"Link\">Link</a>"));
    assert!(html.contains("<a href=\"#tag\" class=\"tag\">#tag</a>"));
}

#[test]
fn test_tag_surrounded_by_text() {
    let markdown = "prefix #tag suffix";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("prefix "));
    assert!(html.contains("<a href=\"#tag\" class=\"tag\">#tag</a>"));
    assert!(html.contains(" suffix"));
}
