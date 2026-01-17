use crate::error::{AppError, AppResult};
use regex::Regex;
use serde_json::Value;

/// Parse frontmatter from markdown content
/// Returns (frontmatter_json, content_without_frontmatter)
pub fn parse_frontmatter(content: &str) -> AppResult<(Option<Value>, String)> {
    // Check if content starts with ---
    if !content.starts_with("---") {
        return Ok((None, content.to_string()));
    }

    // Find the closing ---
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Ok((None, content.to_string()));
    }

    // Find the second occurrence of ---
    let mut end_index = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            end_index = Some(i);
            break;
        }
    }

    let end_index = match end_index {
        Some(idx) => idx,
        None => return Ok((None, content.to_string())), // No closing ---, treat as regular content
    };

    // Extract YAML content between the --- markers
    let yaml_lines = &lines[1..end_index];
    let yaml_content = yaml_lines.join("\n");

    // Parse YAML to JSON
    let frontmatter: Value = serde_yaml::from_str(&yaml_content)
        .map_err(|e| AppError::InvalidInput(format!("Invalid YAML frontmatter: {}", e)))?;

    // Get the rest of the content (after the closing ---)
    let remaining_content = if end_index + 1 < lines.len() {
        lines[end_index + 1..].join("\n")
    } else {
        String::new()
    };

    Ok((Some(frontmatter), remaining_content))
}

/// Serialize frontmatter and combine with content
pub fn serialize_frontmatter(frontmatter: Option<&Value>, content: &str) -> AppResult<String> {
    match frontmatter {
        None => Ok(content.to_string()),
        Some(fm) => {
            // Convert JSON back to YAML
            let yaml_str = serde_yaml::to_string(fm)
                .map_err(|e| AppError::InternalError(format!("Failed to serialize frontmatter: {}", e)))?;

            // Combine with content
            Ok(format!("---\n{}---\n{}", yaml_str, content))
        }
    }
}

/// Extract tags from frontmatter and content
pub fn extract_tags(frontmatter: Option<&Value>, content: &str) -> Vec<String> {
    let mut tags = Vec::new();

    // Extract tags from frontmatter
    if let Some(fm) = frontmatter {
        if let Some(tags_value) = fm.get("tags") {
            match tags_value {
                Value::Array(arr) => {
                    for item in arr {
                        if let Some(tag) = item.as_str() {
                            tags.push(tag.to_string());
                        }
                    }
                }
                Value::String(s) => {
                    // Single tag as string
                    tags.push(s.clone());
                }
                _ => {}
            }
        }
    }

    // Extract inline tags from content (#tag format)
    let tag_regex = Regex::new(r"#([a-zA-Z0-9_-]+)").unwrap();
    for cap in tag_regex.captures_iter(content) {
        if let Some(tag) = cap.get(1) {
            let tag_str = tag.as_str().to_string();
            if !tags.contains(&tag_str) {
                tags.push(tag_str);
            }
        }
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
title: Test Note
tags:
  - test
  - example
date: 2024-01-01
---
# Heading

Some content here"#;

        let (frontmatter, remaining) = parse_frontmatter(content).unwrap();
        assert!(frontmatter.is_some());
        assert_eq!(remaining.trim(), "# Heading\n\nSome content here");

        let fm = frontmatter.unwrap();
        assert_eq!(fm["title"], "Test Note");
    }

    #[test]
    fn test_no_frontmatter() {
        let content = "# Just a heading\n\nNo frontmatter here";
        let (frontmatter, remaining) = parse_frontmatter(content).unwrap();
        assert!(frontmatter.is_none());
        assert_eq!(remaining, content);
    }

    #[test]
    fn test_extract_tags() {
        let fm_json = serde_json::json!({
            "tags": ["yaml-tag", "another"]
        });
        let content = "Some content with #inline-tag and #another-tag";

        let tags = extract_tags(Some(&fm_json), content);
        assert!(tags.contains(&"yaml-tag".to_string()));
        assert!(tags.contains(&"inline-tag".to_string()));
    }
}
