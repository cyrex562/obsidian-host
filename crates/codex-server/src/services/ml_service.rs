use crate::models::{
    NoteOutlineResponse, OrganizationSuggestion, OrganizationSuggestionKind,
    OrganizationSuggestionsResponse, OutlineSection,
};
use crate::services::frontmatter_service;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashSet;

pub struct MlService;

impl MlService {
    pub fn generate_outline(
        file_path: &str,
        content: &str,
        max_sections: usize,
    ) -> NoteOutlineResponse {
        let capped_max_sections = max_sections.clamp(1, 100);

        let mut sections = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            let Some((level, title)) = Self::parse_heading(line) else {
                continue;
            };

            sections.push(OutlineSection {
                level,
                title: title.to_string(),
                line_number: idx + 1,
            });

            if sections.len() >= capped_max_sections {
                break;
            }
        }

        let summary = Self::build_summary(content, sections.first().map(|s| s.title.as_str()));

        NoteOutlineResponse {
            file_path: file_path.to_string(),
            summary,
            sections,
            generated_at: Utc::now(),
        }
    }

    pub fn suggest_organization(
        file_path: &str,
        content: &str,
        frontmatter: Option<&Value>,
        max_suggestions: usize,
    ) -> OrganizationSuggestionsResponse {
        let capped_max_suggestions = max_suggestions.clamp(1, 25);
        let content_lower = content.to_lowercase();

        let existing_tags = frontmatter_service::extract_tags(frontmatter, content);
        let existing_tag_set: HashSet<String> = existing_tags
            .iter()
            .map(|t| t.trim().trim_start_matches('#').to_lowercase())
            .collect();

        let mut suggestions: Vec<OrganizationSuggestion> = Vec::new();

        let tag_rules: [(&[&str], &str, &str, f32); 7] = [
            (
                &["meeting", "agenda", "minutes"],
                "meeting",
                "Detected meeting-related terms in the note content.",
                0.86,
            ),
            (
                &["todo", "task", "action item", "checklist"],
                "tasks",
                "Detected task-oriented language suggesting task tracking.",
                0.89,
            ),
            (
                &["project", "milestone", "roadmap", "deliverable"],
                "project",
                "Detected project planning terminology in this note.",
                0.84,
            ),
            (
                &["bug", "issue", "fix", "regression"],
                "bug",
                "Detected bug/issue language in this note.",
                0.83,
            ),
            (
                &["idea", "brainstorm", "concept", "proposal"],
                "idea",
                "Detected ideation terms suggesting an idea note.",
                0.8,
            ),
            (
                &["daily", "journal", "reflection", "log"],
                "daily",
                "Detected journaling language in this note.",
                0.78,
            ),
            (
                &["research", "experiment", "analysis", "hypothesis"],
                "research",
                "Detected research terminology in this note.",
                0.81,
            ),
        ];

        for (keywords, tag, rationale, confidence) in tag_rules {
            let has_keyword = keywords.iter().any(|k| content_lower.contains(k));
            if !has_keyword {
                continue;
            }

            if existing_tag_set.contains(tag) {
                continue;
            }

            suggestions.push(OrganizationSuggestion {
                id: format!("tag:{}", tag),
                kind: OrganizationSuggestionKind::Tag,
                confidence,
                rationale: rationale.to_string(),
                tag: Some(tag.to_string()),
                category: None,
                target_folder: None,
            });
        }

        let inferred_category = Self::infer_category(file_path, &content_lower);
        if let Some(category) = inferred_category {
            let existing_category = frontmatter
                .and_then(|fm| fm.get("category"))
                .and_then(Value::as_str)
                .map(|s| s.to_lowercase());

            if existing_category.as_deref() != Some(category) {
                suggestions.push(OrganizationSuggestion {
                    id: format!("category:{}", category),
                    kind: OrganizationSuggestionKind::Category,
                    confidence: 0.76,
                    rationale: "Suggested from note path and content semantics.".to_string(),
                    tag: None,
                    category: Some(category.to_string()),
                    target_folder: None,
                });
            }

            let target_folder = Self::folder_for_category(category);
            let normalized_path = file_path.replace('\\', "/").to_lowercase();
            if !normalized_path.starts_with(&target_folder.to_lowercase()) {
                suggestions.push(OrganizationSuggestion {
                    id: format!("move:{}", target_folder),
                    kind: OrganizationSuggestionKind::MoveToFolder,
                    confidence: 0.7,
                    rationale: "Path appears inconsistent with inferred category; move is suggested for organization only.".to_string(),
                    tag: None,
                    category: Some(category.to_string()),
                    target_folder: Some(target_folder),
                });
            }
        }

        suggestions.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        suggestions.truncate(capped_max_suggestions);

        OrganizationSuggestionsResponse {
            file_path: file_path.to_string(),
            suggestions,
            existing_tags,
            generated_at: Utc::now(),
        }
    }

    fn parse_heading(line: &str) -> Option<(u8, &str)> {
        let trimmed = line.trim_start();
        let hashes_len = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes_len == 0 || hashes_len > 6 {
            return None;
        }

        let remainder = trimmed[hashes_len..].trim();
        if remainder.is_empty() {
            return None;
        }

        Some((hashes_len as u8, remainder))
    }

    fn build_summary(content: &str, first_heading: Option<&str>) -> String {
        let mut fragments = Vec::new();

        if let Some(title) = first_heading {
            fragments.push(format!("Focus: {}.", title));
        }

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                continue;
            }

            fragments.push(trimmed.to_string());
            if fragments.len() >= 3 {
                break;
            }
        }

        let mut summary = fragments.join(" ");
        if summary.is_empty() {
            summary =
                "No substantial prose found; add content for a richer outline summary.".to_string();
        }

        if summary.len() > 260 {
            summary.truncate(257);
            summary.push_str("...");
        }

        summary
    }

    fn infer_category(file_path: &str, content_lower: &str) -> Option<&'static str> {
        let normalized_path = file_path.replace('\\', "/").to_lowercase();

        if normalized_path.contains("daily") || normalized_path.contains("journal") {
            return Some("journal");
        }
        if normalized_path.contains("meeting") {
            return Some("meetings");
        }
        if normalized_path.contains("project") {
            return Some("projects");
        }
        if normalized_path.contains("task") || normalized_path.contains("todo") {
            return Some("tasks");
        }

        if content_lower.contains("meeting") || content_lower.contains("agenda") {
            return Some("meetings");
        }
        if content_lower.contains("daily") || content_lower.contains("journal") {
            return Some("journal");
        }
        if content_lower.contains("project") || content_lower.contains("milestone") {
            return Some("projects");
        }
        if content_lower.contains("todo") || content_lower.contains("checklist") {
            return Some("tasks");
        }

        None
    }

    fn folder_for_category(category: &str) -> String {
        match category {
            "journal" => "journal/".to_string(),
            "meetings" => "meetings/".to_string(),
            "projects" => "projects/".to_string(),
            "tasks" => "tasks/".to_string(),
            other => format!("{}/", other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MlService;
    use crate::models::OrganizationSuggestionKind;

    #[test]
    fn outline_extracts_headings_and_summary() {
        let content = "# Project Alpha\n\nA short intro line.\n\n## Goals\n- item\n\n### Notes\nMore details.";
        let outline = MlService::generate_outline("projects/alpha.md", content, 10);

        assert_eq!(outline.file_path, "projects/alpha.md");
        assert_eq!(outline.sections.len(), 3);
        assert_eq!(outline.sections[0].level, 1);
        assert_eq!(outline.sections[0].title, "Project Alpha");
        assert!(outline.summary.to_lowercase().contains("project alpha"));
    }

    #[test]
    fn suggestions_include_tag_and_folder_move() {
        let content =
            "# Sprint Meeting\n\nAgenda:\n- Discuss project roadmap\n- Action items and todo list";
        let suggestions =
            MlService::suggest_organization("inbox/sprint-meeting.md", content, None, 10);

        assert!(!suggestions.suggestions.is_empty());
        assert!(suggestions
            .suggestions
            .iter()
            .any(|s| matches!(s.kind, OrganizationSuggestionKind::Tag)
                && s.tag.as_deref() == Some("meeting")));
        assert!(suggestions
            .suggestions
            .iter()
            .any(|s| matches!(s.kind, OrganizationSuggestionKind::MoveToFolder)));
    }
}
