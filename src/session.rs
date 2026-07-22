use std::collections::HashMap;

use tokio::{
    select,
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use tokio_util::task::JoinMap;

use crate::{
    ClientMessage, ErrorMessage, ServerMessage, SessionId,
    game::GameMessage,
    lobby::{
        ConnectionTx, HostMessage, InternalLobbyMessage, LeftLobbyReason, LobbyCode, LobbyMessage,
        lobby_task,
    },
};

pub async fn lobby_coordinator_task(
    mut lobby_coordinator_rx: UnboundedReceiver<LobbyCoordinatorMessage>,
) {
    let mut active_lobbies: JoinMap<LobbyCode, _> = JoinMap::new();
    let mut lobby_senders: HashMap<LobbyCode, UnboundedSender<InternalLobbyMessage>> =
        HashMap::new();
    let mut session_lobby_map: HashMap<SessionId, LobbyCode> = HashMap::new();
    // contains all active sessions, where disconnected ones are automatically removed
    let mut active_sessions: HashMap<SessionId, ConnectionTx> = HashMap::new();

    // for removing a player from the session_lobby_map if they've been disconnected for too long
    let (remove_session_tx, mut remove_session_rx) = mpsc::unbounded_channel();

    'runtime: loop {
        select! {
            biased; // so all patterns below run in order prioritized

            // remove lobbies which task has ended (those lobbies which are closed)
            Some((lobby_code, _result)) = active_lobbies.join_next(), if !active_lobbies.is_empty() => {
                lobby_senders.remove(&lobby_code);
                session_lobby_map.retain(|_session, lobby| lobby != &lobby_code);
            }

            // if a session has been disconnected from the lobby for too long, and removed from a lobby, the lobby sends this signal
            Some(session_id) = remove_session_rx.recv() => {
                session_lobby_map.remove(&session_id);
            }

            Some(message) = lobby_coordinator_rx.recv() => {
                match message {
                    LobbyCoordinatorMessage::SessionConnected { session_id, connection_tx } => {
                        active_sessions.insert(session_id, connection_tx.clone());
                        if let Some(lobby_code) = session_lobby_map.get(&session_id) {
                            lobby_senders.get(lobby_code).unwrap().send(InternalLobbyMessage::PlayerConnected { session_id, connection_tx })
                                    .inspect_err(|e| tracing::error!(error = %e));
                        }
                    }
                    LobbyCoordinatorMessage::SessionDisconnected(session_id) => {
                        active_sessions.remove(&session_id);
                        if let Some(lobby_code) = session_lobby_map.get(&session_id) {
                            lobby_senders.get(lobby_code).unwrap().send(InternalLobbyMessage::PlayerOffline(session_id))
                                    .inspect_err(|e| tracing::error!(error = %e));
                        }
                    }
                    LobbyCoordinatorMessage::ClientMessage { session_id, message } => {
                        let connection_tx = active_sessions.get(&session_id).unwrap().clone();
                        match message {
                            ClientMessage::JoinLobby(lobby_code) => {
                                if let Some(_lobby) = session_lobby_map.get(&session_id) {
                                    connection_tx.send(ServerMessage::Error(ErrorMessage::AlreadyInLobby))
                                        .inspect_err(|e| tracing::error!(error = %e));
                                    continue 'runtime;
                                }
                                let Some(sender) = lobby_senders.get(&lobby_code) else {
                                    connection_tx.send(ServerMessage::Error(ErrorMessage::LobbyDoesNotExist))
                                        .inspect_err(|e| tracing::error!(error = %e));
                                    continue 'runtime;
                                };
                                sender.send(InternalLobbyMessage::PlayerConnected{session_id: session_id, connection_tx: connection_tx.clone()});
                                session_lobby_map.insert(session_id, lobby_code.clone());
                            },
                            ClientMessage::HostLobby => {
                                if let Some(_lobby) = session_lobby_map.get(&session_id) {
                                    connection_tx.send(ServerMessage::Error(ErrorMessage::AlreadyInLobby))
                                        .inspect_err(|e| tracing::error!(error = %e));
                                    continue 'runtime;
                                }

                                // generate new lobby code without collisions
                                let lobby_code = loop {
                                    let code = LobbyCode::new();
                                    if !active_lobbies.contains_key(&code) {
                                        break code;
                                    }
                                };

                                let (lobby_tx, lobby_rx) = mpsc::unbounded_channel();
                                active_lobbies.spawn(lobby_code.clone(), lobby_task(lobby_code.clone(), session_id, connection_tx.clone(), lobby_rx, lobby_tx.clone(), remove_session_tx.clone()));
                                lobby_senders.insert(lobby_code.clone(), lobby_tx);
                                session_lobby_map.insert(session_id, lobby_code.clone());
                            },
                            ClientMessage::LeaveLobby => {
                                let Some(lobby_code) = session_lobby_map.get(&session_id) else {
                                    connection_tx.send(ServerMessage::Error(ErrorMessage::NotInLobby))
                                        .inspect_err(|e| tracing::error!(error = %e));
                                    continue 'runtime;
                                };
                                let sender = lobby_senders.get(lobby_code).unwrap();
                                sender.send(InternalLobbyMessage::PlayerLeft(session_id));
                                session_lobby_map.remove(&session_id);
                                connection_tx.send(ServerMessage::LeftLobby(LeftLobbyReason::Left))
                                        .inspect_err(|e| tracing::error!(error = %e));
                            }
                            message => {
                                // transfer flattened client message to lobby message
                                let message = match message {
                                    ClientMessage::StartGame => LobbyMessage::HostMessage(HostMessage::StartGame),
                                    ClientMessage::StartMatch => LobbyMessage::HostMessage(HostMessage::StartMatch),
                                    ClientMessage::StartRound => LobbyMessage::HostMessage(HostMessage::StartRound),
                                    ClientMessage::TransferHost(player_id) => LobbyMessage::HostMessage(HostMessage::TransferHost(player_id)),
                                    ClientMessage::KickPlayer(player_id) => LobbyMessage::HostMessage(HostMessage::KickPlayer(player_id)),
                                    ClientMessage::CloseLobby => LobbyMessage::HostMessage(HostMessage::CloseLobby),
                                    ClientMessage::SetSettings(lobby_settings) => LobbyMessage::HostMessage(HostMessage::SetSettings(lobby_settings)),
                                    ClientMessage::PerformAction(input_player_action) => LobbyMessage::GameMessage(GameMessage::ActionPerformed(input_player_action)),
                                    ClientMessage::AddNewRule => LobbyMessage::GameMessage(GameMessage::AddNewRule),
                                    ClientMessage::RemoveRule(id) => LobbyMessage::GameMessage(GameMessage::RemoveRule(id)),
                                    _ => {panic!("impossible!")}
                                };
                                let Some(lobby_code) = session_lobby_map.get(&session_id) else {
                                    connection_tx.send(ServerMessage::Error(ErrorMessage::NotInLobby))
                                        .inspect_err(|e| tracing::error!(error = %e));
                                    continue 'runtime;
                                };
                                let sender = lobby_senders.get(lobby_code).unwrap();
                                sender.send(InternalLobbyMessage::LobbyMessage { session_id, message })
                                        .inspect_err(|e| tracing::error!(error = %e));
                            }
                        }
                    }
                    LobbyCoordinatorMessage::ServerShutdown => {
                        for connection_tx in active_sessions.values() {
                            connection_tx.send(ServerMessage::ServerClosed);
                        }
                        break 'runtime;
                    }
                }
            }
        }
    }
}

pub enum LobbyCoordinatorMessage {
    SessionConnected {
        session_id: SessionId,
        connection_tx: ConnectionTx,
    },
    SessionDisconnected(SessionId),
    ClientMessage {
        session_id: SessionId,
        message: ClientMessage,
    },
    ServerShutdown,
}
