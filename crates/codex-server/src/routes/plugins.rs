use crate::services::PluginService;
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

#[derive(Deserialize)]
struct PluginActionRequest {
    enabled: bool,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/api/plugins").route(web::get().to(list_plugins)))
        .service(
            web::resource("/api/plugins/{plugin_id}")
                .route(web::get().to(get_plugin)),
        )
        .service(
            web::resource("/api/plugins/{plugin_id}/config")
                .route(web::put().to(update_plugin_config)),
        )
        .service(
            web::resource("/api/plugins/{plugin_id}/toggle").route(web::post().to(toggle_plugin)),
        )
        .service(
            web::resource("/api/plugins/{plugin_id}/assets/{filename:.*}")
                .route(web::get().to(serve_plugin_file)),
        );
}

async fn list_plugins() -> impl Responder {
    let mut service = PluginService::new("./plugins");
    let plugins = match service.discover_plugins() {
        Ok(plugins) => plugins,
        Err(e) => {
            tracing::error!("Failed to discover plugins: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to discover plugins",
                "plugins": []
            }));
        }
    };
    HttpResponse::Ok().json(json!({ "plugins": plugins }))
}

async fn toggle_plugin(
    path: web::Path<String>,
    req: web::Json<PluginActionRequest>,
) -> impl Responder {
    let plugin_id = path.into_inner();
    let mut service = PluginService::new("./plugins");

    // Discover plugins first
    if let Err(e) = service.discover_plugins() {
        tracing::error!("Failed to discover plugins: {}", e);
        return HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to discover plugins: {}", e)
        }));
    }

    let result = if req.enabled {
        service.enable_plugin(&plugin_id)
    } else {
        service.disable_plugin(&plugin_id)
    };

    match result {
        Ok(_) => HttpResponse::Ok().json(json!({
            "success": true,
            "plugin_id": plugin_id,
            "enabled": req.enabled
        })),
        Err(e) => {
            tracing::error!("Failed to toggle plugin {}: {}", plugin_id, e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to toggle plugin: {}", e)
            }))
        }
    }
}

/// Get a single plugin by ID (includes manifest, config, and config_schema).
async fn get_plugin(path: web::Path<String>) -> impl Responder {
    let plugin_id = path.into_inner();
    let mut service = PluginService::new("./plugins");
    if let Err(e) = service.discover_plugins() {
        return HttpResponse::InternalServerError()
            .json(json!({ "error": format!("Plugin discovery failed: {e}") }));
    }
    match service.get_plugin(&plugin_id) {
        Some(plugin) => HttpResponse::Ok().json(plugin),
        None => HttpResponse::NotFound().json(json!({ "error": "Plugin not found" })),
    }
}

/// Replace a plugin's stored configuration (validated against config_schema if present).
async fn update_plugin_config(
    path: web::Path<String>,
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    let plugin_id = path.into_inner();
    let mut service = PluginService::new("./plugins");
    if let Err(e) = service.discover_plugins() {
        return HttpResponse::InternalServerError()
            .json(json!({ "error": format!("Plugin discovery failed: {e}") }));
    }
    match service.update_plugin_config(&plugin_id, body.into_inner()) {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(e) => HttpResponse::BadRequest().json(json!({ "error": e.to_string() })),
    }
}

/// Serve a static asset from a plugin's `dist/` directory.
///
/// Resolves `./plugins/{plugin_id}/dist/{filename}` and canonicalizes the path
/// to prevent directory traversal attacks.  Returns 404 for missing files and
/// 403 if the resolved path escapes the expected base directory.
async fn serve_plugin_file(
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (plugin_id, filename) = path.into_inner();

    // Reject obvious traversal attempts before touching the filesystem
    if filename.contains("..") {
        return HttpResponse::Forbidden().body("Invalid path");
    }

    let base = Path::new("./plugins")
        .join(&plugin_id)
        .join("dist");

    let file_path = base.join(&filename);

    // Canonicalize both paths so we can compare them
    let Ok(canonical_base) = base.canonicalize() else {
        return HttpResponse::NotFound().body("Plugin not found");
    };
    let Ok(canonical_file) = file_path.canonicalize() else {
        return HttpResponse::NotFound().body("Asset not found");
    };

    // Ensure the file is actually inside the plugin's dist directory
    if !canonical_file.starts_with(&canonical_base) {
        return HttpResponse::Forbidden().body("Access denied");
    }

    match std::fs::read(&canonical_file) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&canonical_file)
                .first_or_octet_stream();
            HttpResponse::Ok()
                .content_type(mime.as_ref())
                .body(bytes)
        }
        Err(_) => HttpResponse::NotFound().body("Asset not found"),
    }
}
