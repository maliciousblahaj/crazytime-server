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
    lobby::{Lobby, LobbyCode, LobbyMessage},
    player::SessionId,
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
        mpsc::{self, Sender, UnboundedSender, error::SendError},
    },
    task::JoinHandle,
    time::sleep,
};

#[derive(Clone)]
struct AppState {
    active_lobbies: HashMap<LobbyCode, ActiveLobbyHandle>,
    session_lobby_index: HashMap<SessionId, LobbyCode>,
}

impl AppState {
    fn new() -> Self {
        Self {
            active_lobbies: DashMap::new(),
            session_lobby_index: HashMap::new(),
        }
    }

    /// host a lobby. returns its lobbycode if succeeded
    ///
    /// returns Err(code) if you're already in a lobby with a certain code
    async fn host_lobby(&mut self, session_id: SessionId) -> Result<LobbyCode, LobbyCode> {
        if let Some(lobby_code) = self.session_lobby_index.get(&session_id) {
            return Err(lobby_code);
        }

        // generate new lobby code without collisions
        let lobby_code = loop {
            let code = LobbyCode::new();
            if !self.active_lobbies.contains_key(&code) {
                break code;
            }
        };

        let lobby = Lobby::new(lobby_code, session_id);
        self.active_lobbies.insert(lobby.lobby_code, lobby);
        self.session_lobby_index
            .insert(session_id, lobby.lobby_code);
        lobby_code
    }

    async fn handle_message(
        &mut self,
        session_id: SessionId,
        message: SessionMessage,
        tx: UnboundedSender<ServerMessage>,
    ) -> Result<(), SendError<ServerMessage>> {
        // TODOOOOOOO
        match message {
            SessionMessage::JoinLobby(lobby_code) => {
                if let Some(lobby) = self.session_lobby_index.get(&session_id) {
                    tx.send(ServerMessage::Error(ErrorMessage::AlreadyInLobby))
                        .await?;
                    return;
                }
                let Some(handle) = self.active_lobbies.get(&lobby_code) else {
                    tx.send(ServerMessage::Error(ErrorMessage::LobbyDoesNotExist))
                        .await?;
                    return;
                };
            }
            SessionMessage::HostLobby => todo!(),
            SessionMessage::LobbyMessage(lobby_message) => {
                let Some(lobby_code) = self.session_lobby_index.get(&session_id) else {
                    tx.send(ServerMessage::Error(ErrorMessage::NotInLobby))
                        .await?;
                    return;
                };
                if let Err(e) = self.send_lobby_message(session_id, lobby_message, tx) {
                    match e {
                        SendLobbyMessageError::LobbyDoesNotExist => {
                            tx.send(ServerMessage::Error(ErrorMessage::LobbyDoesNotExist))
                                .await?;
                            return;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    async fn get_lobby_sender(
        &mut self,
        session_id: SessionId,
    ) -> Option<UnboundedSender<LobbyMessage>> {
        let lobby_code = self.session_lobby_index.get(&session_id)?;
    }

    async fn alert_lobby_of_disconnect(&mut self, session_id: SessionId) {
        todo!()
    }
    async fn connection_added(
        &mut self,
        session_id: SessionId,
        tx: UnboundedSender<ServerMessage>,
    ) {
        todo!()
    }
}

struct ActiveLobbyHandle {
    handle: JoinHandle<()>,
    sender: UnboundedSender<LobbyMessage>,
}

enum JoinLobbyError {
    LobbyDoesNotExist,
    LobbyIsFull,
    AlreadyInLobby(LobbyCode),
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
    player_id: SessionId,
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
        let session_id = input.player_id;
        let (tx, rx) = mpsc::unbounded_channel();

        let idle_duration = Duration::from_secs(20);
        let pong_wait_duration = Duration::from_secs(10);
        let idle_timer = sleep(idle_duration);
        tokio::pin!(idle_timer);
        let mut pong_wait_timer: Option<Pin<Box<Sleep>>> = None;

        // so it can receive appropriate welcome messages
        app_state.connection_added(session_id, tx.clone()).await;
        loop {
            select! {
                // this polls the idle timer, and if it's ready (finished sleeping), this will run
                _ = &mut idle_timer, if pong_wait_timer.is_none() => {
                    socket.send(ws::Message::Ping(())).await.inspect_err(|e| tracing::error!(error = %e, "ping send failed"));
                    pong_wait_timer = Some(Box::pin(sleep(pong_wait_duration)));
                },
                // async block is needed becuase select! expects a future, and this is an Option<impl Future>
                _ = async { pong_wait_timer.as_mut().unwrap().await }, if pong_wait_timer.is_some() => {
                    tracing::warn!(?session_id, "pong timeout, disconnecting");
                    break;
                },
                ws_message = socket.recv() => {
                    idle_timer.as_mut().reset(Instant::now() + idle_duration);
                    match ws_message {
                        Some(Ok(ws::Message::Text(text))) => {
                            let Ok(message) = serde_json::from_str::<SessionMessage>(text) else {
                                tx.send(ServerMessage::Error(ErrorMessage::InvalidClientMessage)).inspect_err(|e| tracing::error!(error = %e));
                                continue;
                            };
                            app_state.handle_message(session_id, message, tx.clone()).await.inspect_err(|e| tracing::error!(error = %e));
                        },
                        Some(Ok(ws::Message::Ping(_))) => {
                            socket.send(ws::Message::Pong(())).await.inspect_err(|e| tracing::error!(error = %e));
                        }
                        Some(Ok(ws::Message::Pong(_))) => {
                            pong_wait_timer = None;
                        }
                        Some(Ok(ws::Message::Close(_))) | None => {
                            // the connection closed. user might later reconnect by starting a new connection, in which the
                            // AppState::connection_added method has them covered to recover the session
                            //
                            // btw message the lobby in some way so it can know to start a wait timer before it disconnects the player
                            app_state.alert_lobby_of_disconnect(session_id).await.inspect_err(|e| tracing::error!(error = %e));
                            break;
                        }
                        Some(_) => {
                            tx.send(ServerMessage::Error(ErrorMessage::InvalidClientMessage)).inspect_err(|e| tracing::error!(error = %e));
                        }
                    }
                },
                server_message = rx.recv() => {
                    match server_message{
                        Some(ServerMessage::Ping) => {
                            socket.send(ws::Message::Ping(())).await.inspect_err(|e| tracing::error!(error = %e));
                        }
                        Some(message) => {
                            let json = serde_json::to_string(&message).unwrap();
                            socket.send(ws::Message::Text(json)).await.inspect_err(|e| tracing::error!(error = %e));
                        },
                        None => {
                            println!("impossible!");
                        },
                    }
                }
            }
        }
    })
}

#[serde(rename_all = "camelCase")]
#[derive(Deserialize)]
pub enum SessionMessage {
    JoinLobby(LobbyCode),
    HostLobby,
    LobbyMessage(LobbyMessage),
}
