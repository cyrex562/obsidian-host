use crate::models::FileChangeEvent;
use crate::routes::vaults::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse, Error};
use actix_ws::Message;
use tokio::sync::broadcast;
use tracing::info;

#[get("/api/ws")]
async fn websocket(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let mut event_rx = state.event_broadcaster.subscribe();

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
                    if let Ok(json) = serde_json::to_string(&change_event) {
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
