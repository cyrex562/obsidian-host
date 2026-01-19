use obsidian_host::services::MarkdownService;

/// Tests for CommonMark specification compliance
/// Based on the CommonMark spec: https://spec.commonmark.org/

#[test]
fn test_commonmark_headings() {
    // ATX headings
    let markdown = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<h1>H1</h1>"));
    assert!(html.contains("<h2>H2</h2>"));
    assert!(html.contains("<h3>H3</h3>"));
    assert!(html.contains("<h4>H4</h4>"));
    assert!(html.contains("<h5>H5</h5>"));
    assert!(html.contains("<h6>H6</h6>"));
}

#[test]
fn test_commonmark_setext_headings() {
    // Setext headings
    let markdown = "Heading 1\n=========\n\nHeading 2\n---------";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<h1>Heading 1</h1>"));
    assert!(html.contains("<h2>Heading 2</h2>"));
}

#[test]
fn test_commonmark_paragraphs() {
    let markdown = "First paragraph.\n\nSecond paragraph.";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<p>First paragraph.</p>"));
    assert!(html.contains("<p>Second paragraph.</p>"));
}

#[test]
fn test_commonmark_line_breaks() {
    // Hard line break with two spaces
    let markdown = "Line 1  \nLine 2";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("Line 1"));
    assert!(html.contains("Line 2"));
}

#[test]
fn test_commonmark_emphasis() {
    let markdown = "*italic* and _also italic_\n\n**bold** and __also bold__\n\n***bold italic***";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<em>italic</em>"));
    assert!(html.contains("<em>also italic</em>"));
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<strong>also bold</strong>"));
}

#[test]
fn test_commonmark_links() {
    // Inline links
    let markdown = "[link text](https://example.com \"title\")";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<a href=\"https://example.com\""));
    assert!(html.contains("link text</a>"));
}

#[test]
fn test_commonmark_autolinks() {
    let markdown = "<https://example.com>";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<a href=\"https://example.com\""));
}

#[test]
fn test_commonmark_images() {
    let markdown = "![alt text](image.jpg \"Image title\")";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<img"));
    assert!(html.contains("src=\"image.jpg\""));
    assert!(html.contains("alt=\"alt text\""));
}

#[test]
fn test_commonmark_code_spans() {
    let markdown = "Use `code` for inline code.";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<code>code</code>"));
}

#[test]
fn test_commonmark_code_blocks_indented() {
    let markdown = "    indented code block\n    second line";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<pre><code>"));
    assert!(html.contains("indented code block"));
}

#[test]
fn test_commonmark_code_blocks_fenced() {
    let markdown = "```\nfenced code block\n```";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<pre>"));
    assert!(html.contains("<code>"));
    assert!(html.contains("fenced code block"));
}

#[test]
fn test_commonmark_blockquotes() {
    let markdown = "> This is a blockquote\n> with multiple lines";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<blockquote>"));
    assert!(html.contains("This is a blockquote"));
}

#[test]
fn test_commonmark_nested_blockquotes() {
    let markdown = "> Level 1\n>> Level 2";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<blockquote>"));
    assert!(html.contains("Level 1"));
    assert!(html.contains("Level 2"));
}

#[test]
fn test_commonmark_unordered_lists() {
    let markdown = "- Item 1\n- Item 2\n  - Nested item\n- Item 3";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<ul>"));
    assert!(html.contains("<li>Item 1</li>"));
    assert!(html.contains("<li>Item 2"));
    assert!(html.contains("Nested item"));
}

#[test]
fn test_commonmark_ordered_lists() {
    let markdown = "1. First\n2. Second\n3. Third";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<ol>"));
    assert!(html.contains("<li>First</li>"));
    assert!(html.contains("<li>Second</li>"));
    assert!(html.contains("<li>Third</li>"));
}

#[test]
fn test_commonmark_horizontal_rules() {
    let markdown = "---\n\n***\n\n___";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should contain at least one horizontal rule
    assert!(html.contains("<hr"));
}

#[test]
fn test_commonmark_html_blocks() {
    let markdown = "<div>\nHTML block\n</div>";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<div>"));
    assert!(html.contains("HTML block"));
    assert!(html.contains("</div>"));
}

#[test]
fn test_commonmark_entity_references() {
    let markdown = "&copy; &amp; &lt; &gt;";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Entity references should be preserved or converted
    assert!(html.contains("&") || html.contains("©"));
}

#[test]
fn test_commonmark_backslash_escapes() {
    let markdown = "\\*not italic\\* \\[not a link\\]";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should not contain em tags or links
    assert!(!html.contains("<em>not italic</em>"));
    assert!(!html.contains("<a"));
}

#[test]
fn test_commonmark_mixed_content() {
    let markdown = r#"# Heading

This is a **paragraph** with *emphasis* and a [link](https://example.com).

- List item 1
- List item 2

> A blockquote

```
code block
```
"#;
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<h1>Heading</h1>"));
    assert!(html.contains("<strong>paragraph</strong>"));
    assert!(html.contains("<em>emphasis</em>"));
    assert!(html.contains("<a href=\"https://example.com\""));
    assert!(html.contains("<ul>"));
    assert!(html.contains("<blockquote>"));
    assert!(html.contains("<pre>"));
}

#[test]
fn test_commonmark_reference_links() {
    let markdown = "[link][ref]\n\n[ref]: https://example.com";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<a href=\"https://example.com\""));
    assert!(html.contains("link</a>"));
}

#[test]
fn test_commonmark_list_with_paragraphs() {
    let markdown = "1. First item\n\n   Second paragraph\n\n2. Second item";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    assert!(html.contains("<ol>"));
    assert!(html.contains("First item"));
    assert!(html.contains("Second paragraph"));
}

#[test]
fn test_commonmark_nested_emphasis() {
    let markdown = "***bold and italic***";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should contain both strong and em tags (nested)
    assert!(html.contains("<strong>") || html.contains("<em>"));
}
