use crate::config::AppConfig;
use crate::middleware::AuthenticatedUser;
use crate::routes::vaults::AppState;
use actix_web::{get, web, Error, HttpMessage, HttpRequest, HttpResponse};
use actix_ws::Message;
use tracing::info;

#[get("/api/ws")]
async fn websocket(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let mut event_rx = state.event_broadcaster.subscribe();
    let auth_enabled = config.auth.enabled;
    let current_user = req.extensions().get::<AuthenticatedUser>().cloned();

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                // Receive messages from the client
                Some(Ok(msg)) = msg_stream.recv() => {
                    match msg {
                        Message::Ping(bytes) => {
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Message::Text(text) => {
                            info!("Received text message: {}", text);
                        }
                        Message::Close(_) => {
                            break;
                        }
                        _ => {}
                    }
                }

                // Receive file change events
                Ok(change_event) = event_rx.recv() => {
                    if auth_enabled {
                        let Some(current_user) = &current_user else {
                            continue;
                        };

                        match state.db.get_vault_role_for_user(&change_event.vault_id, &current_user.user_id).await {
                            Ok(Some(_)) => {}
                            _ => continue,
                        }
                    }

                    let etag = match &change_event.event_type {
                        crate::models::FileChangeType::Created | crate::models::FileChangeType::Modified => {
                            match state.db.get_vault(&change_event.vault_id).await {
                                Ok(vault) => crate::services::FileService::read_file(&vault.path, &change_event.path)
                                    .ok()
                                    .map(|content| format!("\"{:x}\"", content.modified.timestamp_millis())),
                                Err(_) => None,
                            }
                        }
                        _ => None,
                    };

                    let message = crate::models::WsMessage::FileChanged {
                        vault_id: change_event.vault_id.clone(),
                        path: change_event.path.clone(),
                        event_type: change_event.event_type.clone(),
                        etag,
                        timestamp: change_event.timestamp.timestamp_millis(),
                    };

                    if let Ok(json) = serde_json::to_string(&message) {
                        if session.text(json).await.is_err() {
                            break;
                        }
                    }
                }

                else => break,
            }
        }

        let _ = session.close(None).await;
    });

    Ok(response)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(websocket);
}
