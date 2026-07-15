use std::net::{IpAddr, SocketAddr};

use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade},
    response::IntoResponse,
    routing::post,
};
use crazytime_server::{
    lobby::{Lobby, LobbyCode},
    player::PlayerId,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

// TODO: this is how we do the backend:
// first of all you send a normal http request to a join lobby endpoint, or a host endpoint.
// If it errors it will return the error but if it suceeds it will upgrade it to a websocket
// connection, and start talking in messages. It will also send an init message, which contains
// a ton of relevant data, like your assigned player id which you'll save in localstorage,
// and more. If you're the host you can send certain ws requests which youd playerid will authenticate.

// btw i will also make a websocket request which fetches all data for syncing and making sure the
// frontend is at the same state as the backend. obviously it will be, but this is just for testing
// purposes, and since the frontend is vibecoded we must know instantly if it gets something wrong

#[derive(Clone)]
pub struct AppState {
    active_lobbies: DashMap<LobbyCode, Lobby>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active_lobbies: DashMap::new(),
        }
    }

    pub fn host_lobby(&mut self, player_id: PlayerId) -> LobbyCode {
        // TODO check if player already inside a lobby, if so don't let them host

        let lobby_code = loop {
            let code = LobbyCode::new();
            if !self.active_lobbies.contains_key(&code) {
                break code;
            }
        };

        let lobby = Lobby::new(lobby_code, player_id);
        self.active_lobbies.insert(lobby_code, lobby);
        lobby_code
    }
    pub fn join_lobby(
        &mut self,
        player_id: PlayerId,
        lobby_code: LobbyCode,
    ) -> Result<(), JoinLobbyError> {
        // TODO: check if player already inside a lobby, if so don't let them join
        let mut lobby = self
            .active_lobbies
            .get_mut(&lobby_code)
            .ok_or(JoinLobbyError::LobbyDoesNotExist)?;
        *lobby.add
    }
}
pub enum JoinLobbyError {
    LobbyDoesNotExist,
    LobbyIsFull,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinLobbyForm {
    lobby_code: LobbyCode,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let state = AppState::new();

    let app = Router::new()
        .route("/game", get(ws_endpoint))
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
pub struct WsAuthForm {
    player_id: PlayerId,
}

pub fn ws_endpoint(
    State(app): State<AppState>,
    Json(input): Json<WsAuthForm>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        transport::run_connection(socket, input.player_id).await;
    })
}
