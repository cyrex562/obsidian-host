use crate::services::wiki_link_service::{FileIndex, WikiLinkResolver};
use pulldown_cmark::{html, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::sync::LazyLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
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

/// Options for rendering markdown with wiki link resolution
pub struct RenderOptions<'a> {
    /// Vault path for resolving wiki links
    pub vault_path: Option<&'a str>,
    /// Current file path for relative link resolution
    pub current_file: Option<&'a str>,
    /// Pre-built file index for faster resolution
    pub file_index: Option<&'a FileIndex>,
    /// Whether to enable syntax highlighting
    pub enable_highlighting: bool,
}

impl Default for RenderOptions<'_> {
    fn default() -> Self {
        Self {
            vault_path: None,
            current_file: None,
            file_index: None,
            enable_highlighting: true,
        }
    }
}

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
            return Self::parse_with_wiki_links(markdown, options, false, None);
        }

        Self::parse_with_wiki_links(markdown, options, true, None)
    }

    /// Convert markdown to HTML with full link resolution
    pub fn to_html_with_link_resolution(markdown: &str, render_opts: &RenderOptions) -> String {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

        Self::parse_with_wiki_links(
            markdown,
            options,
            render_opts.enable_highlighting,
            Some(render_opts),
        )
    }

    fn parse_with_wiki_links(
        markdown: &str,
        options: Options,
        enable_highlighting: bool,
        render_opts: Option<&RenderOptions>,
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
                Self::process_obsidian_syntax(&text_buffer, &mut html_output, render_opts);
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
                Event::Html(html_content) => {
                    // Escape raw HTML to prevent XSS
                    let escaped = Self::html_escape(&html_content);
                    html_output.push_str(&escaped);
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
            Self::process_obsidian_syntax(&text_buffer, &mut html_output, render_opts);
        }

        html_output
    }

    fn process_obsidian_syntax(
        text: &str,
        html_output: &mut String,
        render_opts: Option<&RenderOptions>,
    ) {
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
                let link_url = &cap[2];
                let link_text = if let Some(alias) = cap.get(3) {
                    alias.as_str().to_string()
                } else {
                    link_url.replace('#', " > ")
                };

                // Resolve the wiki link to an actual file path if render options are provided
                let (resolved_url, link_exists) =
                    Self::resolve_wiki_link_url(link_url, render_opts);

                // Add CSS class based on whether link exists
                let link_class = if link_exists {
                    "wiki-link"
                } else {
                    "wiki-link broken-link"
                };

                if is_embed {
                    // For embeds (images), use raw HTML to include data attributes
                    let escaped_url = Self::html_escape(&resolved_url);
                    let escaped_text = Self::html_escape(&link_text);
                    let html = format!(
                        "<img src=\"{}\" alt=\"{}\" class=\"wiki-embed\" data-original-link=\"{}\" />",
                        escaped_url, escaped_text, Self::html_escape(link_url)
                    );
                    html::push_html(html_output, vec![Event::Html(html.into())].into_iter());
                } else {
                    // For links, use raw HTML to include CSS class and data attributes
                    let escaped_url = Self::html_escape(&resolved_url);
                    let escaped_text = Self::html_escape(&link_text);
                    let html = format!(
                        "<a href=\"{}\" class=\"{}\" data-original-link=\"{}\">{}</a>",
                        escaped_url,
                        link_class,
                        Self::html_escape(link_url),
                        escaped_text
                    );
                    html::push_html(html_output, vec![Event::Html(html.into())].into_iter());
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

    /// Resolve a wiki link to a URL, returning (url, exists)
    fn resolve_wiki_link_url(link: &str, render_opts: Option<&RenderOptions>) -> (String, bool) {
        // Extract fragment if present
        let (base_link, fragment) = if let Some(hash_pos) = link.find('#') {
            (&link[..hash_pos], Some(&link[hash_pos..]))
        } else {
            (link, None)
        };

        // If we have render options with vault path, try to resolve the link
        if let Some(opts) = render_opts {
            if let Some(vault_path) = opts.vault_path {
                // Try using file index first (faster)
                let resolved = if let Some(index) = opts.file_index {
                    index.resolve(base_link)
                } else if let Some(current_file) = opts.current_file {
                    // Use relative resolution
                    WikiLinkResolver::resolve_relative(vault_path, base_link, current_file)
                        .unwrap_or_else(|_| crate::services::wiki_link_service::ResolvedLink {
                            path: format!("{}.md", base_link),
                            exists: false,
                            alternatives: vec![],
                        })
                } else {
                    // Use standard resolution
                    WikiLinkResolver::resolve(vault_path, base_link).unwrap_or_else(|_| {
                        crate::services::wiki_link_service::ResolvedLink {
                            path: format!("{}.md", base_link),
                            exists: false,
                            alternatives: vec![],
                        }
                    })
                };

                // Build the URL with fragment
                let url = if let Some(frag) = fragment {
                    format!("{}{}", resolved.path, frag)
                } else {
                    resolved.path.clone()
                };

                return (Self::percent_encode_path(&url), resolved.exists);
            }
        }

        // No resolution available, return the original link percent-encoded
        let url = if let Some(frag) = fragment {
            format!("{}{}", base_link, frag)
        } else {
            base_link.to_string()
        };

        (Self::percent_encode_path(&url), true) // Assume exists if we can't check
    }

    /// Percent-encode a path for use in URLs
    fn percent_encode_path(path: &str) -> String {
        let mut result = String::with_capacity(path.len() * 2);
        for c in path.chars() {
            match c {
                ' ' => result.push_str("%20"),
                '?' => result.push_str("%3F"),
                // Keep these characters as-is for path readability
                // We MUST keep '#' as-is because it's the fragment separator
                '/' | '-' | '_' | '.' | '~' | '#' | '^' => result.push(c),
                // Keep alphanumeric as-is
                c if c.is_ascii_alphanumeric() => result.push(c),
                // Encode everything else
                c => {
                    for byte in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", byte));
                    }
                }
            }
        }
        result
    }

    /// HTML escape special characters
    fn html_escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
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
                // Text before the tag
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
        assert!(html.contains("Header 1"));
        assert!(html.contains("Cell 1"));
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

        assert!(html.contains("image.png"));
        assert!(html.contains("Alt text"));
    }
}
