use axum::{
    Router,
    extract::{Query, State, WebSocketUpgrade, ws},
    response::IntoResponse,
    routing::get,
};
use crazytime_server::{
    ClientMessage, ErrorMessage, ServerMessage, SessionId,
    session::{LobbyCoordinatorMessage, lobby_coordinator_task},
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, pin::Pin, time::Duration};
use tokio::{
    select,
    sync::mpsc::{self, UnboundedSender},
    time::{Instant, Sleep, sleep},
};

#[derive(Clone)]
struct AppState {
    lobby_coordinator_tx: UnboundedSender<LobbyCoordinatorMessage>,
}

impl AppState {
    fn new(lobby_coordinator_tx: UnboundedSender<LobbyCoordinatorMessage>) -> Self {
        Self {
            lobby_coordinator_tx,
        }
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let (lobby_coordinator_tx, lobby_coordinator_rx) = mpsc::unbounded_channel();
    let lobby_coordinator = tokio::spawn(lobby_coordinator_task(lobby_coordinator_rx));
    let state = AppState::new(lobby_coordinator_tx.clone());

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
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .inspect_err(|e| tracing::error!(error = %e, "ping send failed"))
        .ok();
    lobby_coordinator_tx
        .send(LobbyCoordinatorMessage::ServerShutdown)
        .inspect_err(|e| tracing::error!(error = %e, "ping send failed"))
        .ok();
    lobby_coordinator.await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.unwrap();
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
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
    Query(input): Query<WsAuthForm>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(async move |mut socket| {
        let session_id = input.session_id;
        let (connection_tx, mut connection_rx) = mpsc::unbounded_channel();

        let idle_duration = Duration::from_secs(10);
        let pong_wait_duration = Duration::from_secs(5);
        let idle_timer = sleep(idle_duration);
        tokio::pin!(idle_timer);
        let mut pong_wait_timer: Option<Pin<Box<Sleep>>> = None;

        let mut sequential_message_id = 0;

        app_state.lobby_coordinator_tx.send(LobbyCoordinatorMessage::SessionConnected {
            session_id,
            connection_tx: connection_tx.clone()
        })
            .inspect_err(|e| tracing::error!(error = %e)).ok();
        'ws_runtime: loop {
            select! {
                // this polls the idle timer, and if it's ready (finished sleeping), this will run
                _ = &mut idle_timer, if pong_wait_timer.is_none() => {
                    socket.send(ws::Message::Ping(bytes::Bytes::new())).await.inspect_err(|e| tracing::error!(error = %e, "ping send failed")).ok();
                    pong_wait_timer = Some(Box::pin(sleep(pong_wait_duration)));
                }
                // async block is needed becuase select! expects a future, and this is an Option<impl Future>
                // this block is identical as Some(Ok(ws::Message::Close) below)
                _ = async { pong_wait_timer.as_mut().unwrap().await }, if pong_wait_timer.is_some() => {
                    app_state.lobby_coordinator_tx.send(LobbyCoordinatorMessage::SessionDisconnected(session_id)).inspect_err(|e| tracing::error!(error = %e, "ping send failed")).ok();
                    break 'ws_runtime;
                }
                ws_message = socket.recv() => {
                    idle_timer.as_mut().reset(Instant::now() + idle_duration);
                    match ws_message {
                        Some(Ok(ws::Message::Text(text))) => {
                            let Ok(message) = serde_json::from_str::<ClientMessage>(&text) else {
                                connection_tx.send(ServerMessage::Error(ErrorMessage::InvalidClientMessage)).inspect_err(|e| tracing::error!(error = %e)).ok();
                                continue 'ws_runtime;
                            };
                            app_state.lobby_coordinator_tx.send(LobbyCoordinatorMessage::ClientMessage { session_id, message }).inspect_err(|e| tracing::error!(error = %e)).ok();
                        },
                        Some(Ok(ws::Message::Ping(_))) => {
                            socket.send(ws::Message::Pong(bytes::Bytes::new())).await.inspect_err(|e| tracing::error!(error = %e)).ok();
                        }
                        Some(Ok(ws::Message::Pong(_))) => {
                            pong_wait_timer = None;
                        }
                        Some(Ok(ws::Message::Close(_))) | None => {
                            // the connection closed. user might later reconnect by starting a new connection with the same
                            // session_id, in which they can recover their lobby
                            app_state.lobby_coordinator_tx.send(LobbyCoordinatorMessage::SessionDisconnected(session_id)).inspect_err(|e| tracing::error!(error = %e, "ping send failed")).ok();
                            break 'ws_runtime;
                        }
                        Some(_) => {
                            connection_tx.send(ServerMessage::Error(ErrorMessage::InvalidClientMessage)).inspect_err(|e| tracing::error!(error = %e)).ok();
                        }
                    }
                }
                server_message = connection_rx.recv() => {
                    match server_message {
                        Some(message) => {
                            let json = serde_json::to_string(&ServerPacket { sequence_id: sequential_message_id, message }).unwrap();
                            sequential_message_id += 1; // to detect lost packages
                            socket.send(ws::Message::Text(json.into())).await.inspect_err(|e| tracing::error!(error = %e)).ok();
                        }
                        None => {
                            panic!("impossible!");
                        }
                    }
                }
            };
        }
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPacket {
    sequence_id: usize,
    message: ServerMessage,
}
