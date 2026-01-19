use pulldown_cmark::{html, CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::sync::LazyLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

// Lazy-loaded syntax set and theme for performance
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(|| SyntaxSet::load_defaults_newlines());
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(|| ThemeSet::load_defaults());
static WIKI_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(!?)\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap());
static TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\s|^)(#[a-zA-Z0-9_\-/]+)").unwrap());

pub struct MarkdownService;

impl MarkdownService {
    /// Convert markdown to HTML with syntax highlighting
    pub fn to_html(markdown: &str) -> String {
        Self::to_html_with_highlighting(markdown, true)
    }

    /// Convert markdown to HTML with optional syntax highlighting
    pub fn to_html_with_highlighting(markdown: &str, enable_highlighting: bool) -> String {
        // Enable all CommonMark extensions
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

        if !enable_highlighting {
            // Simple rendering without syntax highlighting, but we still need wiki links
            // So we use the same logic but without highlighting
            return Self::parse_with_wiki_links(markdown, options, false);
        }

        Self::parse_with_wiki_links(markdown, options, true)
    }

    fn parse_with_wiki_links(
        markdown: &str,
        options: Options,
        enable_highlighting: bool,
    ) -> String {
        // Parse markdown and apply syntax highlighting to code blocks
        let parser = Parser::new_ext(markdown, options);
        let mut html_output = String::new();

        // Use cached syntax set and theme
        let theme = &THEME_SET.themes["base16-ocean.dark"];

        let mut in_code_block = false;
        let mut in_frontmatter = false;
        let mut code_block_lang = String::new();
        let mut code_block_content = String::new();
        let mut text_buffer = String::new();

        for event in parser {
            // Handle text buffering for wiki links (outside code blocks and frontmatter)
            if let Event::Text(ref text) = event {
                if in_frontmatter {
                    continue; // Ignore frontmatter text
                }
                if !in_code_block {
                    text_buffer.push_str(text);
                    continue;
                }
            }

            // If we have a non-text event (or text in code block), flush the buffer first
            if !text_buffer.is_empty() {
                Self::process_obsidian_syntax(&text_buffer, &mut html_output);
                text_buffer.clear();
            }

            match event {
                Event::Start(Tag::MetadataBlock(_)) => {
                    in_frontmatter = true;
                }
                Event::End(TagEnd::MetadataBlock(_)) => {
                    in_frontmatter = false;
                }
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                    in_code_block = true;
                    code_block_lang = lang.to_string();
                    code_block_content.clear();
                }

                Event::End(TagEnd::CodeBlock) if in_code_block => {
                    in_code_block = false;

                    if enable_highlighting {
                        // Apply syntax highlighting
                        let highlighted =
                            Self::highlight_code(&code_block_content, &code_block_lang, theme);
                        html_output.push_str(&highlighted);
                    } else {
                        html_output.push_str("<pre><code>");
                        let escaped = code_block_content
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;");
                        html_output.push_str(&escaped);
                        html_output.push_str("</code></pre>\n");
                    }
                }
                Event::Text(text) if in_code_block => {
                    code_block_content.push_str(&text);
                }
                _ => {
                    // For non-code-block events, use default HTML rendering
                    let single_event = vec![event];
                    html::push_html(&mut html_output, single_event.into_iter());
                }
            }
        }

        // Flush remaining buffer at end
        if !text_buffer.is_empty() {
            Self::process_obsidian_syntax(&text_buffer, &mut html_output);
        }

        html_output
    }

    fn process_obsidian_syntax(text: &str, html_output: &mut String) {
        if WIKI_LINK_REGEX.is_match(text) {
            let mut last_end = 0;

            for cap in WIKI_LINK_REGEX.captures_iter(text) {
                let match_start = cap.get(0).unwrap().start();
                let match_end = cap.get(0).unwrap().end();

                // Text before the link (process tags in it)
                if match_start > last_end {
                    let segment = &text[last_end..match_start];
                    Self::process_tags(segment, html_output);
                }

                // The link itself
                let is_embed = &cap[1] == "!";
                // ... (rest of link logic) ...
                let link_url = &cap[2];
                let link_text = if let Some(alias) = cap.get(3) {
                    alias.as_str().to_string()
                } else {
                    link_url.replace('#', " > ")
                };

                // Flush anything pushed by process_tags directly to html_output?
                // Wait, process_tags pushes to html_output.
                // But here I'm collecting `events` vector?
                // Note: The previous implementation pushed to `events` vector then `push_html` at the end.
                // If `process_tags` calls `push_html` directly, I have ordering issues if I mix `events` vector.
                // Refactor: `process_obsidian_syntax` should prob NOT use `events` vector but push directly to avoid mixing strategies.
                // Or `process_tags` can return events?
                // simpler: `process_tags` pushes to html_output. Link logic pushes to html_output.

                // Let's rewrite `process_obsidian_syntax` to push directly.

                if is_embed {
                    html::push_html(
                        html_output,
                        vec![
                            Event::Start(Tag::Image {
                                link_type: LinkType::Inline,
                                dest_url: link_url.to_string().into(),
                                title: "".into(),
                                id: "".into(),
                            }),
                            Event::Text(link_text.to_string().into()),
                            Event::End(TagEnd::Image),
                        ]
                        .into_iter(),
                    );
                } else {
                    html::push_html(
                        html_output,
                        vec![
                            Event::Start(Tag::Link {
                                link_type: LinkType::Inline,
                                dest_url: link_url.to_string().into(),
                                title: "".into(),
                                id: "".into(),
                            }),
                            Event::Text(link_text.to_string().into()),
                            Event::End(TagEnd::Link),
                        ]
                        .into_iter(),
                    );
                }

                last_end = match_end;
            }

            // Text after the last link
            if last_end < text.len() {
                let segment = &text[last_end..];
                Self::process_tags(segment, html_output);
            }
        } else {
            // No wiki links, just process tags
            Self::process_tags(text, html_output);
        }
    }

    fn process_tags(text: &str, html_output: &mut String) {
        if TAG_REGEX.is_match(text) {
            let mut last_end = 0;
            // Iterate tags
            for cap in TAG_REGEX.captures_iter(text) {
                let full_match = cap.get(0).unwrap();
                let tag_match = cap.get(1).unwrap(); // The tag including #

                // Check if purely numeric
                let tag_content = &tag_match.as_str()[1..]; // skip #
                if tag_content.chars().all(|c| c.is_numeric()) {
                    continue; // Skip numeric tags, treated as text
                }

                // Text before the tag
                // Note: full_match start might include the space `\s`
                let match_start = full_match.start();
                let match_end = full_match.end();

                // If capture group 1 start > full match start, there is a space
                let tag_start = tag_match.start();

                // Push text before the actual tag (including the space if capturing group didn't include it, but our regex `(?:\s|^)` is non-capturing)
                // Wait `(?:\s|^)` consumes the space.
                // `tag_match` is group 1 `(#[...])`.
                // So text from `last_end` to `tag_start` is the text + space.

                if tag_start > last_end {
                    let prefix = &text[last_end..tag_start];
                    html::push_html(
                        html_output,
                        vec![Event::Text(prefix.to_string().into())].into_iter(),
                    );
                }

                // The tag
                let tag_text = tag_match.as_str();
                // Render as link or span?
                // <a href="#tag" class="tag">#tag</a>
                // Construct raw HTML for tag to add class "tag" easily?
                // pulldown-cmark doesn't have Tag type.
                // Event::Html is easiest.

                let tag_html = format!("<a href=\"{}\" class=\"tag\">{}</a>", tag_text, tag_text);
                html::push_html(html_output, vec![Event::Html(tag_html.into())].into_iter());

                last_end = match_end;
            }

            if last_end < text.len() {
                html::push_html(
                    html_output,
                    vec![Event::Text(text[last_end..].to_string().into())].into_iter(),
                );
            }
        } else {
            html::push_html(
                html_output,
                vec![Event::Text(text.to_string().into())].into_iter(),
            );
        }
    }

    /// Highlight code using syntect
    fn highlight_code(code: &str, lang: &str, theme: &syntect::highlighting::Theme) -> String {
        let syntax = SYNTAX_SET
            .find_syntax_by_token(lang)
            .or_else(|| SYNTAX_SET.find_syntax_by_extension(lang))
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

        let mut html = String::from("<pre><code>");

        let mut highlighter = HighlightLines::new(syntax, theme);
        for line in LinesWithEndings::from(code) {
            let ranges = highlighter
                .highlight_line(line, &SYNTAX_SET)
                .unwrap_or_default();
            let highlighted = styled_line_to_highlighted_html(&ranges[..], IncludeBackground::No)
                .unwrap_or_else(|_| line.to_string());
            html.push_str(&highlighted);
        }

        html.push_str("</code></pre>\n");
        html
    }

    /// Convert markdown to HTML with custom options
    pub fn to_html_with_options(markdown: &str, enable_smart_punctuation: bool) -> String {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

        if enable_smart_punctuation {
            options.insert(Options::ENABLE_SMART_PUNCTUATION);
        }

        let parser = Parser::new_ext(markdown, options);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        html_output
    }

    /// Extract plain text from markdown (strip formatting)
    pub fn to_plain_text(markdown: &str) -> String {
        let parser = Parser::new(markdown);
        let mut plain_text = String::new();
        let mut last_was_text = false;

        for event in parser {
            use pulldown_cmark::Event::*;
            use pulldown_cmark::TagEnd;
            match event {
                Text(text) | Code(text) => {
                    plain_text.push_str(&text);
                    last_was_text = true;
                }
                SoftBreak | HardBreak => {
                    plain_text.push(' ');
                    last_was_text = false;
                }
                End(TagEnd::Paragraph) | End(TagEnd::Heading(_)) => {
                    if last_was_text {
                        plain_text.push(' ');
                    }
                    last_was_text = false;
                }
                _ => {}
            }
        }

        plain_text.trim().to_string()
    }

    /// Get a preview/excerpt from markdown (first N characters of plain text)
    pub fn get_excerpt(markdown: &str, max_length: usize) -> String {
        let plain = Self::to_plain_text(markdown);
        if plain.len() <= max_length {
            plain
        } else {
            let truncated = &plain[..max_length];
            format!("{}...", truncated.trim_end())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown_to_html() {
        let markdown = "# Hello World\n\nThis is **bold** and *italic*.";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<h1>Hello World</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
    }

    #[test]
    fn test_code_blocks() {
        let markdown = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let html = MarkdownService::to_html(markdown);

        // Should contain code block and the code content
        assert!(html.contains("<pre>") && html.contains("<code>"));
        assert!(html.contains("main"));
        assert!(html.contains("println"));
    }

    #[test]
    fn test_code_blocks_with_syntax_highlighting() {
        let markdown = "```rust\nlet x = 42;\n```";
        let html = MarkdownService::to_html(markdown);

        // Should contain syntax highlighted HTML
        assert!(html.contains("<pre>") && html.contains("<code>"));
        assert!(html.contains("let"));
        assert!(html.contains("42"));
    }

    #[test]
    fn test_code_blocks_without_highlighting() {
        let markdown = "```rust\nfn main() {}\n```";
        let html = MarkdownService::to_html_with_highlighting(markdown, false);

        assert!(html.contains("<pre>") && html.contains("<code>"));
        assert!(html.contains("main"));
    }

    #[test]
    fn test_lists() {
        let markdown = "- Item 1\n- Item 2\n- Item 3";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>Item 1</li>"));
        assert!(html.contains("</ul>"));
    }

    #[test]
    fn test_links() {
        let markdown = "[Google](https://google.com)";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<a href=\"https://google.com\">Google</a>"));
    }

    #[test]
    fn test_tables() {
        let markdown = "| Header 1 | Header 2 |\n|----------|----------|\n| Cell 1   | Cell 2   |";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<table>"));
        assert!(html.contains("<th>Header 1</th>"));
        assert!(html.contains("<td>Cell 1</td>"));
    }

    #[test]
    fn test_strikethrough() {
        let markdown = "~~strikethrough~~";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<del>strikethrough</del>"));
    }

    #[test]
    fn test_task_lists() {
        let markdown = "- [ ] Unchecked\n- [x] Checked";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("type=\"checkbox\""));
        assert!(html.contains("disabled=\"\""));
    }

    #[test]
    fn test_plain_text_extraction() {
        let markdown = "# Title\n\nThis is **bold** text with a [link](url).";
        let plain = MarkdownService::to_plain_text(markdown);

        assert_eq!(plain, "Title This is bold text with a link.");
    }

    #[test]
    fn test_excerpt_generation() {
        let markdown = "# Long Article\n\nThis is a very long article with lots of content that should be truncated.";
        let excerpt = MarkdownService::get_excerpt(markdown, 30);

        assert!(excerpt.len() <= 33); // 30 + "..."
        assert!(excerpt.ends_with("..."));
    }

    #[test]
    fn test_blockquotes() {
        let markdown = "> This is a quote\n> Second line";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<blockquote>"));
        assert!(html.contains("This is a quote"));
    }

    #[test]
    fn test_inline_code() {
        let markdown = "Use `code` for inline code.";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn test_headings() {
        let markdown = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<h1>H1</h1>"));
        assert!(html.contains("<h2>H2</h2>"));
        assert!(html.contains("<h3>H3</h3>"));
        assert!(html.contains("<h4>H4</h4>"));
        assert!(html.contains("<h5>H5</h5>"));
        assert!(html.contains("<h6>H6</h6>"));
    }

    #[test]
    fn test_horizontal_rule() {
        let markdown = "Above\n\n---\n\nBelow";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<hr />"));
    }

    #[test]
    fn test_images() {
        let markdown = "![Alt text](image.png)";
        let html = MarkdownService::to_html(markdown);

        assert!(html.contains("<img src=\"image.png\" alt=\"Alt text\" />"));
    }
}
