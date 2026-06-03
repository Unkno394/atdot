use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{auth::jwt, state::AppState};

#[derive(Deserialize)]
pub struct WsQuery {
    token: Option<String>,
}

pub async fn handler(
    ws:             WebSocketUpgrade,
    Query(query):   Query<WsQuery>,
    State(state):   State<Arc<AppState>>,
) -> impl IntoResponse {
    // Validate JWT if provided — reject unknown clients
    let authed = query.token
        .as_deref()
        .map(|t| jwt::decode_token(t, &state.jwt_secret).is_ok())
        .unwrap_or(false);

    if !authed {
        return ws.on_upgrade(|mut s| async move {
            let _ = s.send(Message::Text(r#"{"error":"unauthorized"}"#.into())).await;
            let _ = s.close().await;
        });
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.ws_tx.subscribe();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if socket.send(Message::Text(text)).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
