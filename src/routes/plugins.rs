use crate::services::PluginService;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
struct PluginActionRequest {
    enabled: bool,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/api/plugins").route(web::get().to(list_plugins)))
        .service(
            web::resource("/api/plugins/{plugin_id}/toggle").route(web::post().to(toggle_plugin)),
        );
    // TODO: Route for serving plugin assets
    // .service(web::resource("/plugins/{plugin_id}/{filename:.*}").route(web::get().to(serve_plugin_file)))
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
