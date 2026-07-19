use axum::{
    Json, Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{self, Message},
    },
    response::IntoResponse,
    routing::post,
};
use crazytime_server::{
    ErrorMessage, ServerMessage, SessionId,
    lobby::{ConnectionTx, Lobby, LobbyCode, LobbyMessage},
    player::SessionId,
    session::{LobbyCoordinatorMessage, lobby_coordinator_task},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::{
    select,
    sync::{
        Mutex,
        mpsc::{self, Sender, UnboundedReceiver, UnboundedSender, error::SendError},
    },
    task::{self, JoinHandle, JoinSet},
    time::sleep,
};
use tokio_util::task::JoinMap;

#[derive(Clone)]
struct AppState {
    lobby_coordinator: JoinHandle<()>,
    lobby_coordinator_tx: UnboundedSender<LobbyCoordinatorMessage>,
}

impl AppState {
    async fn new() -> Self {
        let (lobby_coordinator_tx, lobby_coordinator_rx) = mpsc::unbounded_channel();
        let lobby_coordinator = tokio::spawn(lobby_coordinator_task(lobby_coordinator_rx));
        Self {
            lobby_coordinator,
            lobby_coordinator_tx,
        }
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let state = AppState::new();

    let app = Router::new()
        .route("/api", get(ws_endpoint))
        .route("/get_token", get(get_token_endpoint))
        .with_state(state);

    let port: u16 = std::env::var("CRAZYTIME_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WsAuthForm {
    session_id: SessionId,
}

async fn get_token_endpoint() -> impl IntoResponse {
    SessionId::new().to_string()
}

async fn ws_endpoint(
    State(app_state): State<AppState>,
    Json(input): Json<WsAuthForm>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(async move |mut socket| {
        let session_id = input.session_id;
        let (connection_tx, connection_rx) = mpsc::unbounded_channel();

        let idle_duration = Duration::from_secs(20);
        let pong_wait_duration = Duration::from_secs(10);
        let idle_timer = sleep(idle_duration);
        tokio::pin!(idle_timer);
        let mut pong_wait_timer: Option<Pin<Box<Sleep>>> = None;

        app_state.lobby_coordinator_tx.send(LobbyCoordinatorMessage::SessionConnected {
            session_id: session_id.clone(),
            connection_tx: connection_tx.clone()
        })
            .inspect_err(|e| tracing::error!(error = %e));
        'ws_runtime: loop {
            select! {
                // this polls the idle timer, and if it's ready (finished sleeping), this will run
                _ = &mut idle_timer, if pong_wait_timer.is_none() => {
                    socket.send(ws::Message::Ping(())).await.inspect_err(|e| tracing::error!(error = %e, "ping send failed"));
                    pong_wait_timer = Some(Box::pin(sleep(pong_wait_duration)));
                }
                // async block is needed becuase select! expects a future, and this is an Option<impl Future>
                // this block is identical as Some(Ok(ws::Message::Close) below)
                _ = async { pong_wait_timer.as_mut().unwrap().await }, if pong_wait_timer.is_some() => {
                    app_state.lobby_coordinator_tx.send(LobbyCoordinatorMessage::SessionDisconnected(session_id));
                    break 'ws_runtime;
                }
                ws_message = socket.recv() => {
                    idle_timer.as_mut().reset(Instant::now() + idle_duration);
                    match ws_message {
                        Some(Ok(ws::Message::Text(text))) => {
                            let Ok(message) = serde_json::from_str::<SessionMessage>(text) else {
                                connection_tx.send(ServerMessage::Error(ErrorMessage::InvalidClientMessage)).inspect_err(|e| tracing::error!(error = %e));
                                continue 'ws_runtime;
                            };
                            app_state.lobby_coordinator_tx.send(LobbyCoordinatorMessage::SessionMessage { session_id: session_id.clone(), message }).inspect_err(|e| tracing::error!(error = %e));
                        },
                        Some(Ok(ws::Message::Ping(_))) => {
                            socket.send(ws::Message::Pong(())).await.inspect_err(|e| tracing::error!(error = %e));
                        }
                        Some(Ok(ws::Message::Pong(_))) => {
                            pong_wait_timer = None;
                        }
                        Some(Ok(ws::Message::Close(_))) | None => {
                            // the connection closed. user might later reconnect by starting a new connection with the same
                            // session_id, in which they can recover their lobby
                            app_state.lobby_coordinator_tx.send(LobbyCoordinatorMessage::SessionDisconnected(session_id));
                            break 'ws_runtime;
                        }
                        Some(_) => {
                            connection_tx.send(ServerMessage::Error(ErrorMessage::InvalidClientMessage)).inspect_err(|e| tracing::error!(error = %e));
                        }
                    }
                }
                server_message = connection_rx.recv() => {
                    match server_message {
                        Some(message) => {
                            let json = serde_json::to_string(&message).unwrap();
                            socket.send(ws::Message::Text(json)).await.inspect_err(|e| tracing::error!(error = %e));
                        }
                        None => {
                            panic!("impossible!");
                        }
                    }
                }
            }
        }
    })
}
