use crate::error::AppResult;
use crate::routes::vaults::AppState;
use crate::services::frontmatter_service;
use actix_web::{get, web, HttpResponse};
use serde::Serialize;
use std::collections::HashMap;
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
pub struct TagEntry {
    pub tag: String,
    pub count: usize,
    pub files: Vec<String>,
}

/// GET /api/vaults/{vault_id}/tags
/// Scans all .md files and returns a list of tags with file counts.
#[get("/api/vaults/{vault_id}/tags")]
async fn list_tags(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    // Map tag -> list of files containing that tag
    let mut tag_map: HashMap<String, Vec<String>> = HashMap::new();

    for entry in WalkDir::new(&vault.path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
        })
    {
        let rel_path = entry
            .path()
            .strip_prefix(&vault.path)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");

        if let Ok(raw) = std::fs::read_to_string(entry.path()) {
            let (fm, body) =
                frontmatter_service::parse_frontmatter(&raw).unwrap_or((None, raw.clone()));
            let tags = frontmatter_service::extract_tags(fm.as_ref(), &body);
            for tag in tags {
                tag_map.entry(tag).or_default().push(rel_path.clone());
            }
        }
    }

    let mut entries: Vec<TagEntry> = tag_map
        .into_iter()
        .map(|(tag, mut files)| {
            files.sort();
            let count = files.len();
            TagEntry { tag, count, files }
        })
        .collect();

    entries.sort_by(|a, b| a.tag.to_lowercase().cmp(&b.tag.to_lowercase()));

    Ok(HttpResponse::Ok().json(entries))
}

/// GET /api/vaults/{vault_id}/backlinks?path=notes/hello.md
/// Returns all .md files that contain a wiki-link or markdown link pointing at the given path.
#[get("/api/vaults/{vault_id}/backlinks")]
async fn list_backlinks(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    query: web::Query<BacklinksQuery>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;
    let target_path = query.path.trim();

    // Derive the stem (filename without extension) for wiki-link matching
    let stem = std::path::Path::new(target_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(target_path);

    // Patterns to search for
    let wiki_stem_lower = format!("[[{}]]", stem.to_lowercase());
    let path_lower = target_path.to_lowercase();
    let path_no_ext = target_path.trim_end_matches(".md").to_lowercase();

    #[derive(Serialize)]
    struct BacklinkEntry {
        path: String,
        title: String,
    }

    let mut results: Vec<BacklinkEntry> = Vec::new();

    for entry in WalkDir::new(&vault.path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
        })
    {
        let rel_path = entry
            .path()
            .strip_prefix(&vault.path)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");

        // Don't include the file linking to itself
        if rel_path.to_lowercase() == path_lower {
            continue;
        }

        if let Ok(raw) = std::fs::read_to_string(entry.path()) {
            let lower = raw.to_lowercase();
            // Check for [[stem]] style wiki-link or path-based markdown link
            let found = lower.contains(&wiki_stem_lower)
                || lower.contains(&format!("({})", path_lower))
                || lower.contains(&format!("({})", path_no_ext));
            if found {
                let title = std::path::Path::new(&rel_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&rel_path)
                    .to_string();
                results.push(BacklinkEntry {
                    path: rel_path,
                    title,
                });
            }
        }
    }

    results.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(HttpResponse::Ok().json(results))
}

#[derive(serde::Deserialize)]
struct BacklinksQuery {
    path: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_tags).service(list_backlinks);
}
