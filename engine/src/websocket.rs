use crate::device::DeviceContext;
use crate::filter::FilterQuery;
use crate::log_entry::{DeviceInfo, LogEntry, StatisticsSnapshot};
use crate::recorder::{Recorder, RecorderConfig, RecorderStatus};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{header::ORIGIN, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time;

const MOCK_DEVICE_ID: &str = "mock-device";
const MOCK_DEVICE_NAME: &str = "Mock Device";
const MOCK_LOG_LINE: &str = "07-04 12:34:56.789  1234  5678 I ActivityManager: Mock log line";
const BUFFER_CAPACITY: usize = 1_000_000;
const SNAPSHOT_LIMIT: usize = 5_000;
const ALLOWED_ORIGINS: &[&str] = &["http://127.0.0.1:5173", "http://localhost:5173", "file://"];

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ClientMessage {
    ConnectDevice { device_id: String },
    DisconnectDevice { device_id: String },
    SetFilter { device_id: String, query: String },
    SetSearch {
        device_id: String,
        query: String,
        options: serde_json::Value,
    },
    GetStatistics { device_id: String },
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ServerMessage {
    DeviceList {
        devices: Vec<DeviceInfo>,
    },
    NewLogs {
        device_id: String,
        logs: Vec<LogEntry>,
    },
    LogSnapshot {
        device_id: String,
        logs: Vec<LogEntry>,
    },
    Statistics {
        device_id: String,
        stats: StatisticsSnapshot,
    },
    SearchResults {
        device_id: String,
        matches: Vec<u64>,
    },
    RecorderStatus {
        device_id: String,
        enabled: bool,
        path: Option<String>,
        warning: Option<String>,
    },
    Error {
        message: String,
    },
}

pub async fn run_server() -> anyhow::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let app = Router::new().route("/ws", get(ws_handler));

    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("websocket server error: {error}");
        }
    });

    Ok(port)
}

async fn ws_handler(headers: HeaderMap, ws: WebSocketUpgrade) -> Response {
    let origin = headers.get(ORIGIN).and_then(|value| value.to_str().ok());
    if !is_allowed_origin(origin) {
        return StatusCode::FORBIDDEN.into_response();
    }

    ws.on_upgrade(handle_socket).into_response()
}

fn is_allowed_origin(origin: Option<&str>) -> bool {
    origin.is_none_or(|origin| ALLOWED_ORIGINS.contains(&origin))
}

async fn handle_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let mut device = mock_device_context();
    let mut ticker = time::interval(Duration::from_millis(250));

    if !send_server_message(&mut sender, &device_list_message(&device)).await {
        return;
    }
    if !send_recorder_status(&mut sender, &device).await {
        return;
    }

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if !send_mock_tick(&mut sender, &mut device).await {
                    break;
                }
            }
            incoming = receiver.next() => {
                let Some(incoming) = incoming else {
                    break;
                };

                match incoming {
                    Ok(Message::Text(text)) => {
                        if !handle_client_text(&mut sender, &mut device, &text).await {
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(_) => {}
                    Err(error) => {
                        let message = ServerMessage::Error {
                            message: format!("websocket receive error: {error}"),
                        };
                        let _ = send_server_message(&mut sender, &message).await;
                        break;
                    }
                }
            }
        }
    }
}

async fn handle_client_text(
    sender: &mut SplitSink<WebSocket, Message>,
    device: &mut DeviceContext,
    text: &str,
) -> bool {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(ClientMessage::SetFilter { device_id, query }) => {
            if !ensure_mock_device(sender, &device_id).await {
                return false;
            }
            device.set_filter(FilterQuery::parse(&query));
            send_visible_snapshot(sender, device).await && send_statistics(sender, device).await
        }
        Ok(ClientMessage::SetSearch {
            device_id,
            query,
            options: _,
        }) => {
            if !ensure_mock_device(sender, &device_id).await {
                return false;
            }
            let message = ServerMessage::SearchResults {
                device_id: MOCK_DEVICE_ID.to_string(),
                matches: device.search_visible_sequences(&query),
            };
            send_server_message(sender, &message).await
        }
        Ok(ClientMessage::GetStatistics { device_id }) => {
            if !ensure_mock_device(sender, &device_id).await {
                return false;
            }
            send_statistics(sender, device).await
        }
        Ok(
            ClientMessage::ConnectDevice { device_id }
            | ClientMessage::DisconnectDevice { device_id },
        ) => ensure_mock_device(sender, &device_id).await,
        Err(error) => {
            let message = ServerMessage::Error {
                message: format!("invalid client message: {error}"),
            };
            send_server_message(sender, &message).await
        }
    }
}

async fn ensure_mock_device(sender: &mut SplitSink<WebSocket, Message>, device_id: &str) -> bool {
    if device_id == MOCK_DEVICE_ID {
        return true;
    }

    let message = ServerMessage::Error {
        message: format!("unknown device: {device_id}"),
    };
    send_server_message(sender, &message).await
}

async fn send_mock_tick(
    sender: &mut SplitSink<WebSocket, Message>,
    device: &mut DeviceContext,
) -> bool {
    if let Some(entry) = device.ingest_line(MOCK_LOG_LINE) {
        if !entry.hidden {
            let message = ServerMessage::NewLogs {
                device_id: device.device_id.clone(),
                logs: vec![entry],
            };
            if !send_server_message(sender, &message).await {
                return false;
            }
        }
    }

    if !send_recorder_status(sender, device).await {
        return false;
    }

    send_statistics(sender, device).await
}

async fn send_visible_snapshot(
    sender: &mut SplitSink<WebSocket, Message>,
    device: &DeviceContext,
) -> bool {
    let snapshot = device.latest_visible_snapshot(SNAPSHOT_LIMIT);
    let message = ServerMessage::LogSnapshot {
        device_id: device.device_id.clone(),
        logs: snapshot.logs,
    };
    send_server_message(sender, &message).await
}

async fn send_statistics(
    sender: &mut SplitSink<WebSocket, Message>,
    device: &DeviceContext,
) -> bool {
    let snapshot = device.latest_visible_snapshot(SNAPSHOT_LIMIT);
    let message = ServerMessage::Statistics {
        device_id: device.device_id.clone(),
        stats: snapshot.stats,
    };
    send_server_message(sender, &message).await
}

async fn send_server_message(
    sender: &mut SplitSink<WebSocket, Message>,
    message: &ServerMessage,
) -> bool {
    match serde_json::to_string(message) {
        Ok(payload) => sender.send(Message::Text(payload)).await.is_ok(),
        Err(error) => {
            let fallback = serde_json::json!({
                "type": "error",
                "message": format!("serialize server message failed: {error}"),
            })
            .to_string();
            sender.send(Message::Text(fallback)).await.is_ok()
        }
    }
}

async fn send_recorder_status(
    sender: &mut SplitSink<WebSocket, Message>,
    device: &DeviceContext,
) -> bool {
    let snapshot = device.latest_visible_snapshot(SNAPSHOT_LIMIT);
    let message = recorder_status_message(&device.device_id, snapshot.recorder_status);
    send_server_message(sender, &message).await
}

fn recorder_status_message(device_id: &str, status: RecorderStatus) -> ServerMessage {
    ServerMessage::RecorderStatus {
        device_id: device_id.to_string(),
        enabled: status.enabled,
        path: status.path.map(|path| path.display().to_string()),
        warning: status.warning,
    }
}

fn device_list_message(device: &DeviceContext) -> ServerMessage {
    ServerMessage::DeviceList {
        devices: vec![DeviceInfo {
            device_id: device.device_id.clone(),
            device_name: device.device_name.clone(),
            connected: true,
        }],
    }
}

fn mock_device_context() -> DeviceContext {
    let recorder = Recorder::new(RecorderConfig {
        enabled: true,
        root: PathBuf::from("logs"),
        device_name: MOCK_DEVICE_ID.to_string(),
    });

    DeviceContext::new(
        MOCK_DEVICE_ID.to_string(),
        MOCK_DEVICE_NAME.to_string(),
        BUFFER_CAPACITY,
        recorder,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn origin_allowlist_accepts_dev_packaged_and_non_browser_requests() {
        assert!(is_allowed_origin(None));
        assert!(is_allowed_origin(Some("http://127.0.0.1:5173")));
        assert!(is_allowed_origin(Some("http://localhost:5173")));
        assert!(is_allowed_origin(Some("file://")));
        assert!(!is_allowed_origin(Some("http://evil.example")));
    }

    #[test]
    fn client_message_protocol_uses_snake_case_tag_and_camel_case_fields() {
        let message = serde_json::from_value::<ClientMessage>(json!({
            "type": "connect_device",
            "deviceId": "mock-device"
        }))
        .expect("connect_device should deserialize");
        assert!(
            matches!(message, ClientMessage::ConnectDevice { device_id } if device_id == MOCK_DEVICE_ID)
        );

        let message = serde_json::from_value::<ClientMessage>(json!({
            "type": "disconnect_device",
            "deviceId": "mock-device"
        }))
        .expect("disconnect_device should deserialize");
        assert!(
            matches!(message, ClientMessage::DisconnectDevice { device_id } if device_id == MOCK_DEVICE_ID)
        );

        let message = serde_json::from_value::<ClientMessage>(json!({
            "type": "set_filter",
            "deviceId": "mock-device",
            "query": "level:error"
        }))
        .expect("set_filter should deserialize");
        assert!(
            matches!(message, ClientMessage::SetFilter { device_id, query } if device_id == MOCK_DEVICE_ID && query == "level:error")
        );

        let message = serde_json::from_value::<ClientMessage>(json!({
            "type": "set_search",
            "deviceId": "mock-device",
            "query": "mock",
            "options": {
                "regex": false,
                "caseSensitive": false,
                "wholeWord": false
            }
        }))
        .expect("set_search should deserialize");
        assert!(
            matches!(message, ClientMessage::SetSearch { device_id, query, options } if device_id == MOCK_DEVICE_ID && query == "mock" && options["caseSensitive"] == false)
        );
    }

    #[test]
    fn new_logs_message_uses_camel_case_device_id() {
        let payload = serde_json::to_value(ServerMessage::NewLogs {
            device_id: MOCK_DEVICE_ID.to_string(),
            logs: Vec::new(),
        })
        .expect("new_logs serializes");

        assert_eq!(payload["type"], "new_logs");
        assert_eq!(payload["deviceId"], MOCK_DEVICE_ID);
        assert_eq!(payload["logs"], json!([]));
        assert!(payload.get("device_id").is_none());
    }

    #[test]
    fn log_snapshot_message_uses_replacement_protocol() {
        let payload = serde_json::to_value(ServerMessage::LogSnapshot {
            device_id: MOCK_DEVICE_ID.to_string(),
            logs: Vec::new(),
        })
        .expect("log_snapshot serializes");

        assert_eq!(payload["type"], "log_snapshot");
        assert_eq!(payload["deviceId"], MOCK_DEVICE_ID);
        assert_eq!(payload["logs"], json!([]));
        assert!(payload.get("device_id").is_none());
    }

    #[test]
    fn statistics_message_uses_stats_field() {
        let payload = serde_json::to_value(ServerMessage::Statistics {
            device_id: MOCK_DEVICE_ID.to_string(),
            stats: StatisticsSnapshot::default(),
        })
        .expect("statistics serializes");

        assert_eq!(payload["type"], "statistics");
        assert!(payload.get("stats").is_some());
        assert!(payload.get("statistics").is_none());
    }

    #[test]
    fn search_results_message_uses_camel_case_device_id() {
        let payload = serde_json::to_value(ServerMessage::SearchResults {
            device_id: MOCK_DEVICE_ID.to_string(),
            matches: vec![1, 3],
        })
        .expect("search results serializes");

        assert_eq!(payload["type"], "search_results");
        assert_eq!(payload["deviceId"], MOCK_DEVICE_ID);
        assert_eq!(payload["matches"], json!([1, 3]));
        assert!(payload.get("device_id").is_none());
    }

    #[test]
    fn recorder_status_message_is_flat() {
        let status = RecorderStatus {
            enabled: true,
            path: Some(PathBuf::from("logs/mock.log")),
            warning: Some("disk nearly full".to_string()),
        };
        let payload = serde_json::to_value(recorder_status_message(MOCK_DEVICE_ID, status))
            .expect("recorder status serializes");

        assert_eq!(payload["type"], "recorder_status");
        assert_eq!(payload["deviceId"], MOCK_DEVICE_ID);
        assert!(payload.get("device_id").is_none());
        assert_eq!(payload["enabled"], true);
        assert_eq!(payload["path"], "logs/mock.log");
        assert_eq!(payload["warning"], "disk nearly full");
        assert!(payload.get("status").is_none());
    }
}
