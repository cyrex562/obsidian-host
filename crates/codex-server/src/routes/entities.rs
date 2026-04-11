use crate::routes::AppState;
use crate::services::entity_service::EntityService;
use crate::services::reindex_service::ReindexService;
use crate::services::relation_service::RelationService;
use crate::services::TemplateService;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct EntityListQuery {
    pub entity_type: Option<String>,
    pub label: Option<String>,
    pub plugin: Option<String>,
    pub q: Option<String>,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        // Entities for a vault
        .service(
            web::resource("/api/vaults/{vault_id}/entities").route(web::get().to(list_entities)),
        )
        .service(
            web::resource("/api/vaults/{vault_id}/entities/{entity_id}")
                .route(web::get().to(get_entity)),
        )
        // Relations for an entity
        .service(
            web::resource("/api/vaults/{vault_id}/entities/{entity_id}/relations")
                .route(web::get().to(get_entity_relations)),
        )
        // Full graph data (all entities + relations for a vault)
        .service(web::resource("/api/vaults/{vault_id}/graph").route(web::get().to(get_graph)))
        // Trigger a full reindex
        .service(
            web::resource("/api/vaults/{vault_id}/reindex").route(web::post().to(trigger_reindex)),
        )
        // Entity template (vault-scoped — ?type=<type_id>&plugin=<plugin_id>)
        .service(
            web::resource("/api/vaults/{vault_id}/entity-template")
                .route(web::get().to(get_vault_entity_template)),
        )
        // Entity + relations by file path (?path=<relative_path>)
        .service(
            web::resource("/api/vaults/{vault_id}/entity-by-path")
                .route(web::get().to(get_entity_by_path)),
        )
        // Labels list (also registered here for convenience)
        .service(web::resource("/api/plugins/labels").route(web::get().to(list_labels)))
        // Schema registries
        .service(web::resource("/api/plugins/entity-types").route(web::get().to(list_entity_types)))
        .service(
            web::resource("/api/plugins/entity-types/{type_id}/template")
                .route(web::get().to(get_entity_type_template)),
        )
        .service(
            web::resource("/api/plugins/relation-types").route(web::get().to(list_relation_types)),
        );
}

async fn list_entities(
    path: web::Path<String>,
    query: web::Query<EntityListQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let vault_id = path.into_inner();

    match EntityService::list(
        &state.db,
        &vault_id,
        query.entity_type.as_deref(),
        query.label.as_deref(),
        query.plugin.as_deref(),
        query.q.as_deref(),
    )
    .await
    {
        Ok(entities) => HttpResponse::Ok().json(json!({ "entities": entities })),
        Err(e) => {
            tracing::error!("list_entities error: {e}");
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

async fn get_entity(path: web::Path<(String, String)>, state: web::Data<AppState>) -> HttpResponse {
    let (_vault_id, entity_id) = path.into_inner();

    match EntityService::get(&state.db, &entity_id).await {
        Ok(Some(entity)) => HttpResponse::Ok().json(entity),
        Ok(None) => HttpResponse::NotFound().json(json!({ "error": "Entity not found" })),
        Err(e) => {
            tracing::error!("get_entity error: {e}");
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

async fn get_entity_relations(
    path: web::Path<(String, String)>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let (vault_id, entity_id) = path.into_inner();

    match EntityService::get(&state.db, &entity_id).await {
        Ok(Some(entity)) => {
            match build_entity_relations_payload(&state, &vault_id, &entity).await {
                Ok(relations) => HttpResponse::Ok().json(json!({ "relations": relations })),
                Err(e) => {
                    tracing::error!("get_entity_relations error: {e}");
                    HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
                }
            }
        }
        Ok(None) => HttpResponse::NotFound().json(json!({ "error": "Entity not found" })),
        Err(e) => {
            tracing::error!("get_entity_relations entity lookup error: {e}");
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

/// Returns a graph payload suitable for D3 force simulation:
/// `{ nodes: [Entity], links: [Relation] }`
async fn get_graph(path: web::Path<String>, state: web::Data<AppState>) -> HttpResponse {
    let vault_id = path.into_inner();

    let entities_fut = EntityService::list_all_in_vault(&state.db, &vault_id);
    let relations_fut = RelationService::list_for_vault(&state.db, &vault_id);

    let (entities_res, relations_res) = tokio::join!(entities_fut, relations_fut);

    match (entities_res, relations_res) {
        (Ok(entities), Ok(relations)) => {
            // Shape entities into GraphNode objects
            let nodes: Vec<serde_json::Value> = entities
                .iter()
                .map(|e| {
                    let title = std::path::Path::new(&e.path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&e.path)
                        .to_string();
                    json!({
                        "id": e.id,
                        "path": e.path,
                        "entity_type": e.entity_type,
                        "labels": e.labels_vec(),
                        "title": title,
                    })
                })
                .collect();

            // Shape relations into GraphEdge objects
            let edges: Vec<serde_json::Value> = relations
                .iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "source": r.from_entity_id,
                        "target": r.to_entity_id,
                        "relation_type": r.relation_type,
                        "source_field": r.source_field,
                        "direction": r.direction,
                    })
                })
                .collect();

            HttpResponse::Ok().json(json!({ "nodes": nodes, "edges": edges }))
        }
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!("get_graph error: {e}");
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

async fn trigger_reindex(path: web::Path<String>, state: web::Data<AppState>) -> HttpResponse {
    let vault_id = path.into_inner();

    let vault = match state.db.get_vault(&vault_id).await {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::NotFound()
                .json(json!({ "error": format!("Vault not found: {e}") }));
        }
    };

    // Spawn async so we return immediately (reindex can be slow)
    let db = state.db.clone();
    let ws_tx = state.ws_broadcaster.clone();
    let vid = vault_id.clone();
    let vpath = vault.path.clone();
    tokio::spawn(async move {
        let start = std::time::Instant::now();
        match ReindexService::reindex_vault(&db, &vid, &vpath).await {
            Ok(file_count) => {
                let duration_ms = start.elapsed().as_millis() as i64;
                let msg = crate::models::WsMessage::ReindexComplete {
                    vault_id: vid.clone(),
                    file_count,
                    duration_ms,
                };
                let _ = ws_tx.send(msg);
                tracing::info!(
                    "Reindex complete for vault {vid}: {file_count} files in {duration_ms}ms"
                );
            }
            Err(e) => {
                tracing::error!("Background reindex failed for vault {vid}: {e}");
            }
        }
    });

    HttpResponse::Accepted().json(json!({
        "message": "Reindex started",
        "vault_id": vault_id
    }))
}

async fn list_labels(state: web::Data<AppState>) -> HttpResponse {
    match crate::services::LabelService::list(&state.db).await {
        Ok(labels) => HttpResponse::Ok().json(json!({ "labels": labels })),
        Err(e) => {
            tracing::error!("list_labels error: {e}");
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

async fn list_entity_types(state: web::Data<AppState>) -> HttpResponse {
    let types = state.entity_type_registry.all().await;
    HttpResponse::Ok().json(json!({ "entity_types": types }))
}

async fn get_entity_type_template(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let type_id = path.into_inner();
    match TemplateService::get_template(&state.entity_type_registry, &type_id, &state.plugins_dir)
        .await
    {
        Ok(content) => HttpResponse::Ok().json(json!({ "content": content })),
        Err(e) => {
            tracing::warn!("Template fetch failed for {type_id}: {e}");
            HttpResponse::NotFound().json(json!({ "error": e.to_string() }))
        }
    }
}

async fn list_relation_types(state: web::Data<AppState>) -> HttpResponse {
    let types = state.relation_type_registry.all().await;
    HttpResponse::Ok().json(json!({ "relation_types": types }))
}

#[derive(Deserialize)]
struct EntityTemplateQuery {
    #[serde(rename = "type")]
    entity_type: String,
    #[allow(dead_code)]
    plugin: Option<String>,
}

async fn get_vault_entity_template(
    _path: web::Path<String>,
    query: web::Query<EntityTemplateQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    match TemplateService::get_template(
        &state.entity_type_registry,
        &query.entity_type,
        &state.plugins_dir,
    )
    .await
    {
        Ok(content) => HttpResponse::Ok().json(json!({ "content": content })),
        Err(e) => {
            tracing::warn!(
                "Vault entity template fetch failed for {}: {e}",
                query.entity_type
            );
            HttpResponse::NotFound().json(json!({ "error": e.to_string() }))
        }
    }
}

#[derive(Deserialize)]
struct EntityByPathQuery {
    path: String,
}

async fn get_entity_by_path(
    vault_path_param: web::Path<String>,
    query: web::Query<EntityByPathQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let vault_id = vault_path_param.into_inner();
    match EntityService::get_by_path(&state.db, &vault_id, &query.path).await {
        Ok(Some(entity)) => {
            match build_entity_relations_payload(&state, &vault_id, &entity).await {
                Ok(relations) => HttpResponse::Ok().json(json!({
                    "entity": entity,
                    "relations": relations,
                })),
                Err(e) => {
                    tracing::warn!("get_entity_by_path relations error: {e}");
                    HttpResponse::Ok().json(json!({ "entity": entity, "relations": [] }))
                }
            }
        }
        Ok(None) => HttpResponse::NotFound().json(json!({ "entity": null, "relations": [] })),
        Err(_) => HttpResponse::NotFound().json(json!({ "entity": null, "relations": [] })),
    }
}

async fn build_entity_relations_payload(
    state: &web::Data<AppState>,
    vault_id: &str,
    entity: &crate::services::entity_service::Entity,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let related_entities = EntityService::list_all_in_vault(&state.db, vault_id).await?;
    let path_by_id: HashMap<_, _> = related_entities
        .into_iter()
        .map(|related| (related.id, related.path))
        .collect();

    let relations = RelationService::get_for_entity(&state.db, &entity.id).await?;

    Ok(relations
        .into_iter()
        .filter(|relation| relation.from_entity_id == entity.id)
        .filter_map(|relation| {
            let target_path = path_by_id.get(&relation.to_entity_id)?.clone();
            let metadata = relation
                .metadata
                .as_deref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
                .unwrap_or_else(|| json!({}));

            Some(json!({
                "id": relation.id,
                "source_entity_id": relation.from_entity_id,
                "target_entity_id": relation.to_entity_id,
                "target_path": target_path,
                "relation_type": relation.relation_type,
                "label": relation.relation_type,
                "directed": true,
                "metadata": metadata,
                "plugin_id": serde_json::Value::Null,
                "is_inverse": relation.direction == "inverse",
            }))
        })
        .collect())
}
