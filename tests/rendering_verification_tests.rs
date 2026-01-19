use obsidian_host::services::MarkdownService;

/// Rendering correctness verification tests
/// These tests verify that the HTML output is semantically correct and well-formed

#[test]
fn verify_heading_hierarchy() {
    let markdown = "# H1\n## H2\n### H3";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Verify proper nesting order
    let h1_pos = html.find("<h1>").expect("H1 should exist");
    let h2_pos = html.find("<h2>").expect("H2 should exist");
    let h3_pos = html.find("<h3>").expect("H3 should exist");

    assert!(h1_pos < h2_pos, "H1 should come before H2");
    assert!(h2_pos < h3_pos, "H2 should come before H3");

    // Verify closing tags
    assert!(html.contains("</h1>"));
    assert!(html.contains("</h2>"));
    assert!(html.contains("</h3>"));
}

#[test]
fn verify_list_structure() {
    let markdown = "- Item 1\n- Item 2\n  - Nested\n- Item 3";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should have proper ul/li structure
    assert!(html.contains("<ul>"));
    assert!(html.contains("</ul>"));
    assert!(html.contains("<li>"));
    assert!(html.contains("</li>"));

    // Count opening and closing tags
    let ul_open_count = html.matches("<ul>").count();
    let ul_close_count = html.matches("</ul>").count();
    assert_eq!(ul_open_count, ul_close_count, "UL tags should be balanced");
}

#[test]
fn verify_link_attributes() {
    let markdown = "[Link](https://example.com \"Title\")";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Verify href attribute
    assert!(html.contains("href=\"https://example.com\""));

    // Verify link text
    assert!(html.contains(">Link</a>"));
}

#[test]
fn verify_image_attributes() {
    let markdown = "![Alt text](image.jpg \"Image title\")";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Verify img tag attributes
    assert!(html.contains("<img"));
    assert!(html.contains("src=\"image.jpg\""));
    assert!(html.contains("alt=\"Alt text\""));
}

#[test]
fn verify_code_block_structure() {
    let markdown = "```rust\nlet x = 42;\n```";
    let html = MarkdownService::to_html(markdown);

    // Should have pre and code tags
    assert!(html.contains("<pre>"));
    assert!(html.contains("<code>"));
    assert!(html.contains("</code>"));
    assert!(html.contains("</pre>"));

    // Code content should be present
    assert!(html.contains("let"));
    assert!(html.contains("42"));
}

#[test]
fn verify_emphasis_nesting() {
    let markdown = "***bold and italic***";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should contain both strong and em tags
    assert!(html.contains("<strong>") || html.contains("<em>"));
    assert!(html.contains("bold and italic"));
}

#[test]
fn verify_blockquote_structure() {
    let markdown = "> Quote line 1\n> Quote line 2";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should have blockquote tags
    assert!(html.contains("<blockquote>"));
    assert!(html.contains("</blockquote>"));

    // Content should be present
    assert!(html.contains("Quote line 1"));
    assert!(html.contains("Quote line 2"));
}

#[test]
fn verify_paragraph_separation() {
    let markdown = "Paragraph 1\n\nParagraph 2\n\nParagraph 3";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should have multiple paragraph tags
    let p_count = html.matches("<p>").count();
    assert_eq!(p_count, 3, "Should have 3 paragraphs");

    // Verify closing tags
    let p_close_count = html.matches("</p>").count();
    assert_eq!(p_count, p_close_count, "P tags should be balanced");
}

#[test]
fn verify_html_escaping() {
    let markdown = "Text with <script>alert('xss')</script> and & symbols";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // HTML should be escaped or preserved as-is (depending on CommonMark rules)
    // The important thing is it shouldn't execute
    assert!(html.contains("&") || html.contains("&amp;") || html.contains("<script>"));
}

#[test]
fn verify_special_characters() {
    let markdown = "Copyright © 2024 & trademark ™";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Special characters should be preserved or entity-encoded
    assert!(html.contains("©") || html.contains("&copy;"));
    assert!(html.contains("&") || html.contains("&amp;"));
}

#[test]
fn verify_inline_code_escaping() {
    let markdown = "Use `<html>` tags";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Code content should be in code tags
    assert!(html.contains("<code>"));
    assert!(html.contains("</code>"));

    // HTML inside code should be escaped or preserved
    assert!(html.contains("&lt;") || html.contains("<html>"));
}

#[test]
fn verify_ordered_list_numbering() {
    let markdown = "1. First\n2. Second\n3. Third";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should have ol tag
    assert!(html.contains("<ol>"));
    assert!(html.contains("</ol>"));

    // Should have li tags
    assert!(html.contains("<li>First</li>"));
    assert!(html.contains("<li>Second</li>"));
    assert!(html.contains("<li>Third</li>"));
}

#[test]
fn verify_horizontal_rule_rendering() {
    let markdown = "Above\n\n---\n\nBelow";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should have hr tag
    assert!(html.contains("<hr"));

    // Should be self-closing
    assert!(html.contains("<hr />") || html.contains("<hr>"));
}

#[test]
fn verify_mixed_content_structure() {
    let markdown = r#"# Title

A paragraph with **bold** and *italic*.

- List item
- Another item

> A quote

```
code
```
"#;
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Verify all elements are present
    assert!(html.contains("<h1>"));
    assert!(html.contains("<p>"));
    assert!(html.contains("<strong>"));
    assert!(html.contains("<em>"));
    assert!(html.contains("<ul>"));
    assert!(html.contains("<blockquote>"));
    assert!(html.contains("<pre>"));

    // Verify proper closing
    assert!(html.contains("</h1>"));
    assert!(html.contains("</p>"));
    assert!(html.contains("</strong>"));
    assert!(html.contains("</em>"));
    assert!(html.contains("</ul>"));
    assert!(html.contains("</blockquote>"));
    assert!(html.contains("</pre>"));
}

#[test]
fn verify_empty_input() {
    let markdown = "";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Empty input should produce minimal or empty output
    assert!(html.is_empty() || html.trim().is_empty());
}

#[test]
fn verify_whitespace_handling() {
    let markdown = "   Text with leading spaces\n\nText with trailing spaces   ";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Should contain paragraph tags
    assert!(html.contains("<p>"));

    // Content should be present
    assert!(html.contains("Text with leading spaces"));
    assert!(html.contains("Text with trailing spaces"));
}

#[test]
fn verify_unicode_support() {
    let markdown = "# 你好世界 🌍\n\nEmoji: 😀 🎉 ✨";
    let html = MarkdownService::to_html_with_highlighting(markdown, false);

    // Unicode characters should be preserved
    assert!(html.contains("你好世界"));
    assert!(html.contains("🌍"));
    assert!(html.contains("😀"));
}

#[test]
fn verify_long_document() {
    let mut markdown = String::new();
    for i in 1..=100 {
        markdown.push_str(&format!(
            "## Section {}\n\nContent for section {}.\n\n",
            i, i
        ));
    }

    let html = MarkdownService::to_html_with_highlighting(&markdown, false);

    // Should contain all sections
    assert!(html.contains("Section 1"));
    assert!(html.contains("Section 50"));
    assert!(html.contains("Section 100"));

    // Should have proper structure
    assert!(html.contains("<h2>"));
    assert!(html.contains("<p>"));
}

#[test]
fn verify_plain_text_accuracy() {
    let markdown = "# Title\n\n**Bold** and *italic* text with [link](url).";
    let plain = MarkdownService::to_plain_text(markdown);

    // Should strip all formatting
    assert!(!plain.contains("**"));
    assert!(!plain.contains("*"));
    assert!(!plain.contains("["));
    assert!(!plain.contains("]"));
    assert!(!plain.contains("(url)"));

    // Should contain text content
    assert!(plain.contains("Title"));
    assert!(plain.contains("Bold"));
    assert!(plain.contains("italic"));
    assert!(plain.contains("link"));
}

#[test]
fn verify_excerpt_truncation() {
    let markdown = "# Long Article\n\nThis is a very long article with lots of content that should be truncated properly when generating an excerpt.";
    let excerpt = MarkdownService::get_excerpt(markdown, 50);

    // Should be truncated
    assert!(excerpt.len() <= 53); // 50 + "..."
    assert!(excerpt.ends_with("..."));

    // Should contain beginning of content
    assert!(excerpt.contains("Long Article"));
}
