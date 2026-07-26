use actix_web::{web, HttpRequest, HttpResponse};
use base64::Engine;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::broadcast;

use crate::app_state::AppState;
use crate::handlers::security::is_safe_path;

pub struct NoteRoomManager {
    rooms: Mutex<HashMap<String, broadcast::Sender<Vec<u8>>>>,
}

impl NoteRoomManager {
    pub fn new() -> Self {
        NoteRoomManager {
            rooms: Mutex::new(HashMap::new()),
        }
    }

    pub fn subscribe(&self, room: &str) -> broadcast::Receiver<Vec<u8>> {
        let mut rooms = self.rooms.lock().unwrap_or_else(|e| e.into_inner());
        let sender = rooms
            .entry(room.to_string())
            .or_insert_with(|| broadcast::channel(512).0);
        sender.subscribe()
    }

    pub fn broadcast(&self, room: &str, msg: Vec<u8>) {
        let rooms = self.rooms.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = rooms.get(room) {
            let _ = sender.send(msg);
        }
    }

    pub fn cleanup_empty(&self, room: &str) {
        let mut rooms = self.rooms.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = rooms.get(room) {
            if sender.receiver_count() == 0 {
                rooms.remove(room);
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct NoteWsQuery {
    pub notebook_id: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct SnapshotPayload {
    pub yjs_state: String,
    pub text: String,
}

pub async fn note_ws(
    req: HttpRequest,
    payload: web::Payload,
    app_state: web::Data<AppState>,
    query: web::Query<NoteWsQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let session_id = req
        .cookie("session_id")
        .map(|c| c.value().to_string())
        .unwrap_or_default();

    let user_id = match app_state.session_manager.get(&session_id, "user_id") {
        Some(id) => id,
        None => return Ok(HttpResponse::Unauthorized().finish()),
    };

    let root_path = match app_state.session_manager.get(&session_id, "root_path") {
        Some(p) => p,
        None => return Ok(HttpResponse::Unauthorized().finish()),
    };

    let notebook = match app_state.notebook_model.get_by_id_and_user(&query.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        _ => return Ok(HttpResponse::NotFound().finish()),
    };

    if notebook.encrypted {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let note_path = query.path.trim_matches('/').to_string();
    if !is_safe_path(&note_path) {
        return Ok(HttpResponse::BadRequest().finish());
    }

    let full_path = Path::new(&root_path)
        .join(&notebook.path)
        .join(&note_path);

    if !full_path.exists() {
        return Ok(HttpResponse::NotFound().finish());
    }

    let room_key = match full_path.canonicalize() {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => full_path.to_string_lossy().to_string(),
    };

    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, payload)?;

    let room_manager = Arc::clone(&app_state.note_rooms);
    let note_file_path = full_path.clone();
    let client_id = uuid::Uuid::new_v4().to_string();

    actix_web::rt::spawn(async move {
        let mut rx = room_manager.subscribe(&room_key);

        let yjs_path = note_file_path.with_extension("yjs");
        if let Ok(snapshot) = std::fs::read(&yjs_path) {
            if !snapshot.is_empty() {
                let mut init_msg = Vec::with_capacity(1 + snapshot.len());
                init_msg.push(0x00u8);
                init_msg.extend_from_slice(&snapshot);
                if session.binary(init_msg).await.is_err() {
                    return;
                }
            }
        }

        let session_clone = session.clone();
        let client_id_clone = client_id.clone();
        let forward_task = actix_web::rt::spawn(async move {
            let mut s = session_clone;
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        if msg.len() > 36 && msg[0] == 0xFF {
                            let id_str = String::from_utf8_lossy(&msg[1..37]);
                            if id_str == client_id_clone {
                                continue;
                            }
                            let mut relay = Vec::with_capacity(1 + msg.len() - 37);
                            relay.push(0x00u8);
                            relay.extend_from_slice(&msg[37..]);
                            if s.binary(relay).await.is_err() {
                                break;
                            }
                        } else {
                            if s.binary(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            let _ = s.close(None).await;
        });

        while let Some(Ok(msg)) = futures_util::StreamExt::next(&mut msg_stream).await {
            match msg {
                actix_ws::Message::Binary(bytes) => {
                    if bytes.is_empty() {
                        continue;
                    }
                    match bytes[0] {
                        0x00 => {
                            let update = bytes[1..].to_vec();
                            let mut tagged = Vec::with_capacity(37 + update.len());
                            tagged.push(0xFF);
                            tagged.extend_from_slice(client_id.as_bytes());
                            tagged.extend_from_slice(&update);
                            room_manager.broadcast(&room_key, tagged);
                        }
                        0x01 => {
                            let json_bytes = &bytes[1..];
                            if let Ok(payload) = serde_json::from_slice::<SnapshotPayload>(json_bytes) {
                                if let Ok(yjs_binary) = base64::engine::general_purpose::STANDARD.decode(&payload.yjs_state) {
                                    let _ = std::fs::write(&yjs_path, &yjs_binary);
                                }
                                let _ = std::fs::write(&note_file_path, &payload.text);
                            }
                        }
                        _ => {}
                    }
                }
                actix_ws::Message::Close(_) => break,
                _ => {}
            }
        }

        forward_task.abort();
        let _ = session.close(None).await;
        room_manager.cleanup_empty(&room_key);
    });

    Ok(response)
}
