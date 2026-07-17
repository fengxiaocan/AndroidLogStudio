use crate::device::ExportMode;
use crate::device_manager::{AdbStatus, AdbStatusMode, DeviceManager, MOCK_DEVICE_ID};
use crate::log_entry::{DeviceInfo, LogEntry, StatisticsSnapshot};
use crate::recorder::RecorderStatus;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{header::ORIGIN, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time;

const MOCK_LOG_LINE: &str =
    "07-04 12:34:56.789  1234  5678 I ActivityManager: Mock log line from com.android.systemui";
const SNAPSHOT_LIMIT: usize = 5_000;
const ALLOWED_ORIGINS: &[&str] = &["http://127.0.0.1:5173", "http://localhost:5173", "file://"];

#[derive(Clone)]
struct AppState {
    token: String,
}

#[derive(Debug, Deserialize)]
struct WsQuery {
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ClientMessage {
    ConnectDevice {
        device_id: String,
    },
    DisconnectDevice {
        device_id: String,
    },
    RemoveDevice {
        device_id: String,
    },
    SetFilter {
        device_id: String,
        query: String,
    },
    SetSearch {
        device_id: String,
        query: String,
        options: serde_json::Value,
    },
    GetStatistics {
        device_id: String,
    },
    RefreshDevices,
    ExportLogs {
        device_id: String,
        mode: String,
    },
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
    #[allow(dead_code)]
    AdbStatus {
        available: bool,
        mode: AdbStatusMode,
        path: Option<String>,
        message: String,
    },
    Error {
        message: String,
    },
    ExportReady {
        device_id: String,
        mode: String,
        path: String,
        line_count: usize,
    },
}

pub struct ServerInfo {
    pub port: u16,
}

pub async fn run_server(token: String) -> anyhow::Result<ServerInfo> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(AppState { token });

    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("websocket server error: {error}");
        }
    });

    Ok(ServerInfo { port })
}

async fn ws_handler(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let origin = headers.get(ORIGIN).and_then(|value| value.to_str().ok());
    if !is_allowed_origin(origin) || !is_allowed_token(query.token.as_deref(), &state.token) {
        return StatusCode::FORBIDDEN.into_response();
    }

    ws.on_upgrade(handle_socket).into_response()
}

fn is_allowed_origin(origin: Option<&str>) -> bool {
    origin.is_none_or(|origin| ALLOWED_ORIGINS.contains(&origin))
}

fn is_allowed_token(candidate: Option<&str>, expected: &str) -> bool {
    candidate.is_some_and(|candidate| candidate == expected)
}

async fn handle_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let project_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut manager = DeviceManager::start(&project_root).await;
    let mut ticker = time::interval(Duration::from_millis(250));
    // When no devices are online, re-scan periodically so plugging in a phone
    // is picked up without requiring a manual Refresh click.
    let mut auto_scan = time::interval(Duration::from_secs(3));
    auto_scan.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    // Skip the immediate first tick — start() already scanned once.
    auto_scan.tick().await;

    if !send_server_message(&mut sender, &device_list_message(&manager)).await {
        return;
    }
    if !send_adb_status(&mut sender, manager.adb_status()).await {
        return;
    }
    if !send_startup_recorder_statuses(&mut sender, &manager).await {
        return;
    }

    loop {
        tokio::select! {
            _ = auto_scan.tick() => {
                // Only auto-scan while idle (no ADB devices listed / all offline mock-less).
                // Avoid restarting logcat streams for already-online devices.
                let has_online_adb = manager.device_list().iter().any(|d| {
                    d.source == crate::log_entry::DeviceSource::Adb && d.connected
                });
                if has_online_adb || manager.is_mock_fallback() {
                    continue;
                }
                let before: Vec<_> = manager
                    .device_list()
                    .iter()
                    .map(|d| (d.device_id.clone(), d.connected))
                    .collect();
                manager.refresh(&project_root).await;
                let after: Vec<_> = manager
                    .device_list()
                    .iter()
                    .map(|d| (d.device_id.clone(), d.connected))
                    .collect();
                if before != after || manager.device_list().iter().any(|d| d.connected) {
                    if !send_adb_status(&mut sender, manager.adb_status()).await {
                        break;
                    }
                    if !send_server_message(&mut sender, &device_list_message(&manager)).await {
                        break;
                    }
                    if !send_refresh_device_state(&mut sender, &manager).await {
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                if manager.poll_logcat_exits().await {
                    manager.refresh_adb_status_message();
                    if !send_server_message(&mut sender, &device_list_message(&manager)).await {
                        break;
                    }
                    if !send_adb_status(&mut sender, manager.adb_status()).await {
                        break;
                    }
                }
                if manager.is_mock_fallback() {
                    if !send_mock_tick(&mut sender, &mut manager).await {
                        break;
                    }
                } else if !send_pending_adb_logs(&mut sender, &mut manager).await {
                    break;
                }
            }
            incoming = receiver.next() => {
                let Some(incoming) = incoming else {
                    break;
                };

                match incoming {
                    Ok(Message::Text(text)) => {
                        if !handle_client_text(&mut sender, &mut manager, &text).await {
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(_) => {}
                    Err(error) => {
                        let _ = send_error(&mut sender, format!("websocket receive error: {error}")).await;
                        break;
                    }
                }
            }
        }
    }
}

async fn handle_client_text(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &mut DeviceManager,
    text: &str,
) -> bool {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(ClientMessage::SetFilter { device_id, query }) => {
            if !apply_result(sender, manager.set_filter(&device_id, &query)).await {
                return false;
            }
            send_visible_snapshot(sender, manager, &device_id).await
                && send_statistics(sender, manager, &device_id).await
        }
        Ok(ClientMessage::SetSearch {
            device_id,
            query,
            options: _,
        }) => match manager.search_visible_sequences(&device_id, &query) {
            Ok(matches) => {
                let message = ServerMessage::SearchResults { device_id, matches };
                send_server_message(sender, &message).await
            }
            Err(error) => send_error(sender, error.to_string()).await,
        },
        Ok(ClientMessage::GetStatistics { device_id }) => {
            send_statistics(sender, manager, &device_id).await
        }
        Ok(ClientMessage::RefreshDevices) => {
            let project_root =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            manager.refresh(&project_root).await;
            send_adb_status(sender, manager.adb_status()).await
                && send_server_message(sender, &device_list_message(manager)).await
                && send_refresh_device_state(sender, manager).await
        }
        Ok(ClientMessage::ConnectDevice { device_id }) => {
            if !manager.has_device(&device_id) {
                return send_error(sender, format!("unknown device: {device_id}")).await;
            }
            // Refresh PID cache so the snapshot (and future logs) can include package names
            manager.refresh_pid_caches_if_needed().await;
            send_visible_snapshot(sender, manager, &device_id).await
                && send_statistics(sender, manager, &device_id).await
                && send_recorder_status(sender, manager, &device_id).await
        }
        Ok(ClientMessage::DisconnectDevice { device_id }) => {
            // Intentionally stub this iteration (spec): validate only.
            validate_device(sender, manager, &device_id).await
        }
        Ok(ClientMessage::RemoveDevice { device_id }) => {
            match manager.remove_device(&device_id) {
                Ok(()) => send_server_message(sender, &device_list_message(manager)).await,
                Err(error) => send_error(sender, error.to_string()).await,
            }
        }
        Ok(ClientMessage::ExportLogs { device_id, mode }) => {
            let export_mode = match mode.as_str() {
                "all" => ExportMode::All,
                "filtered" => ExportMode::Filtered,
                other => {
                    return send_error(
                        sender,
                        format!("invalid export mode: {other} (expected all|filtered)"),
                    )
                    .await;
                }
            };
            match manager.export_logs(&device_id, export_mode) {
                Ok(result) => {
                    let mode_label = match result.mode {
                        ExportMode::All => "all",
                        ExportMode::Filtered => "filtered",
                    };
                    send_server_message(
                        sender,
                        &ServerMessage::ExportReady {
                            device_id,
                            mode: mode_label.to_string(),
                            path: result.path.display().to_string(),
                            line_count: result.line_count,
                        },
                    )
                    .await
                }
                Err(error) => send_error(sender, error.to_string()).await,
            }
        }
        Err(error) => send_error(sender, format!("invalid client message: {error}")).await,
    }
}

async fn apply_result(
    sender: &mut SplitSink<WebSocket, Message>,
    result: anyhow::Result<()>,
) -> bool {
    match result {
        Ok(()) => true,
        Err(error) => send_error(sender, error.to_string()).await,
    }
}

async fn validate_device(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &DeviceManager,
    device_id: &str,
) -> bool {
    if manager.has_device(device_id) {
        true
    } else {
        send_error(sender, format!("unknown device: {device_id}")).await
    }
}

async fn send_mock_tick(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &mut DeviceManager,
) -> bool {
    if let Some(entry) = manager.ingest_mock_line(MOCK_LOG_LINE) {
        if !entry.hidden {
            let message = ServerMessage::NewLogs {
                device_id: MOCK_DEVICE_ID.to_string(),
                logs: vec![entry],
            };
            if !send_server_message(sender, &message).await {
                return false;
            }
        }
    }

    send_statistics(sender, manager, MOCK_DEVICE_ID).await
        && send_recorder_status(sender, manager, MOCK_DEVICE_ID).await
}

async fn send_pending_adb_logs(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &mut DeviceManager,
) -> bool {
    manager.refresh_pid_caches_if_needed().await;

    for (device_id, entry) in manager.drain_pending_logs() {
        for message in pending_adb_log_messages(&device_id, entry) {
            match message {
                PendingAdbLogMessage::NewLogs(entry) => {
                    let message = ServerMessage::NewLogs {
                        device_id: device_id.clone(),
                        logs: vec![entry],
                    };
                    if !send_server_message(sender, &message).await {
                        return false;
                    }
                }
                PendingAdbLogMessage::RecorderStatus => {
                    if !send_recorder_status(sender, manager, &device_id).await {
                        return false;
                    }
                }
                PendingAdbLogMessage::Statistics => {
                    if !send_statistics(sender, manager, &device_id).await {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn pending_adb_log_messages(_device_id: &str, entry: LogEntry) -> Vec<PendingAdbLogMessage> {
    let mut messages = Vec::new();
    if !entry.hidden {
        messages.push(PendingAdbLogMessage::NewLogs(entry));
    }
    messages.push(PendingAdbLogMessage::RecorderStatus);
    messages.push(PendingAdbLogMessage::Statistics);
    messages
}

enum PendingAdbLogMessage {
    NewLogs(LogEntry),
    RecorderStatus,
    Statistics,
}

async fn send_visible_snapshot(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &DeviceManager,
    device_id: &str,
) -> bool {
    match manager.latest_visible_snapshot(device_id, SNAPSHOT_LIMIT) {
        Ok(snapshot) => {
            let message = ServerMessage::LogSnapshot {
                device_id: device_id.to_string(),
                logs: snapshot.logs,
            };
            send_server_message(sender, &message).await
        }
        Err(error) => send_error(sender, error.to_string()).await,
    }
}

async fn send_statistics(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &DeviceManager,
    device_id: &str,
) -> bool {
    match manager.latest_visible_snapshot(device_id, SNAPSHOT_LIMIT) {
        Ok(snapshot) => {
            let message = ServerMessage::Statistics {
                device_id: device_id.to_string(),
                stats: snapshot.stats,
            };
            send_server_message(sender, &message).await
        }
        Err(error) => send_error(sender, error.to_string()).await,
    }
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

async fn send_error(
    sender: &mut SplitSink<WebSocket, Message>,
    message: impl Into<String>,
) -> bool {
    send_server_message(
        sender,
        &ServerMessage::Error {
            message: message.into(),
        },
    )
    .await
}

async fn send_adb_status(sender: &mut SplitSink<WebSocket, Message>, status: &AdbStatus) -> bool {
    send_server_message(sender, &adb_status_message(status)).await
}

fn adb_status_message(status: &AdbStatus) -> ServerMessage {
    ServerMessage::AdbStatus {
        available: status.available,
        mode: status.mode.clone(),
        path: status.path.clone(),
        message: status.message.clone(),
    }
}

async fn send_startup_recorder_statuses(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &DeviceManager,
) -> bool {
    for device_id in startup_recorder_status_device_ids(manager) {
        if !send_recorder_status(sender, manager, &device_id).await {
            return false;
        }
    }
    true
}

fn startup_recorder_status_device_ids(manager: &DeviceManager) -> Vec<String> {
    manager
        .device_list()
        .iter()
        .map(|device| device.device_id.clone())
        .collect()
}

async fn send_refresh_device_state(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &DeviceManager,
) -> bool {
    for device_id in refresh_device_state_device_ids(manager) {
        let messages = match refresh_device_state_messages(manager, &device_id) {
            Ok(messages) => messages,
            Err(error) => return send_error(sender, error.to_string()).await,
        };

        for message in messages {
            if !send_server_message(sender, &message).await {
                return false;
            }
        }
    }
    true
}

fn refresh_device_state_device_ids(manager: &DeviceManager) -> Vec<String> {
    manager
        .device_list()
        .iter()
        .map(|device| device.device_id.clone())
        .collect()
}

fn refresh_device_state_messages(
    manager: &DeviceManager,
    device_id: &str,
) -> anyhow::Result<Vec<ServerMessage>> {
    let snapshot = manager.latest_visible_snapshot(device_id, SNAPSHOT_LIMIT)?;
    Ok(vec![
        recorder_status_message(device_id, snapshot.recorder_status),
        ServerMessage::LogSnapshot {
            device_id: device_id.to_string(),
            logs: snapshot.logs,
        },
        ServerMessage::Statistics {
            device_id: device_id.to_string(),
            stats: snapshot.stats,
        },
    ])
}

async fn send_recorder_status(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &DeviceManager,
    device_id: &str,
) -> bool {
    match manager.latest_visible_snapshot(device_id, SNAPSHOT_LIMIT) {
        Ok(snapshot) => {
            let message = recorder_status_message(device_id, snapshot.recorder_status);
            send_server_message(sender, &message).await
        }
        Err(error) => send_error(sender, error.to_string()).await,
    }
}

fn recorder_status_message(device_id: &str, status: RecorderStatus) -> ServerMessage {
    ServerMessage::RecorderStatus {
        device_id: device_id.to_string(),
        enabled: status.enabled,
        path: status.path.map(|path| path.display().to_string()),
        warning: status.warning,
    }
}

fn device_list_message(manager: &DeviceManager) -> ServerMessage {
    ServerMessage::DeviceList {
        devices: manager.device_list().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn origin_allowlist_accepts_dev_packaged_and_non_browser_requests() {
        assert!(is_allowed_origin(None));
        assert!(is_allowed_origin(Some("http://127.0.0.1:5173")));
        assert!(is_allowed_origin(Some("http://localhost:5173")));
        assert!(is_allowed_origin(Some("file://")));
        assert!(!is_allowed_origin(Some("http://evil.example")));
    }

    #[test]
    fn websocket_token_must_match_server_secret() {
        assert!(is_allowed_token(Some("secret"), "secret"));
        assert!(!is_allowed_token(None, "secret"));
        assert!(!is_allowed_token(Some("wrong"), "secret"));
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
    fn refresh_devices_message_deserializes() {
        let message = serde_json::from_value::<ClientMessage>(json!({
            "type": "refresh_devices"
        }))
        .expect("refresh_devices should deserialize");

        assert!(matches!(message, ClientMessage::RefreshDevices));
    }

    #[test]
    fn remove_device_message_deserializes() {
        let message = serde_json::from_value::<ClientMessage>(json!({
            "type": "remove_device",
            "deviceId": "serial-a"
        }))
        .expect("remove_device should deserialize");
        assert!(matches!(
            message,
            ClientMessage::RemoveDevice { device_id } if device_id == "serial-a"
        ));
    }

    #[test]
    fn export_logs_message_deserializes() {
        let message = serde_json::from_value::<ClientMessage>(json!({
            "type": "export_logs",
            "deviceId": "serial-a",
            "mode": "filtered"
        }))
        .expect("export_logs");
        assert!(matches!(
            message,
            ClientMessage::ExportLogs { device_id, mode }
                if device_id == "serial-a" && mode == "filtered"
        ));
    }

    #[test]
    fn export_ready_message_serializes_camel_case() {
        let payload = serde_json::to_value(ServerMessage::ExportReady {
            device_id: "serial-a".into(),
            mode: "all".into(),
            path: "/tmp/x.log".into(),
            line_count: 3,
        })
        .unwrap();
        assert_eq!(payload["type"], "export_ready");
        assert_eq!(payload["deviceId"], "serial-a");
        assert_eq!(payload["lineCount"], 3);
        assert_eq!(payload["path"], "/tmp/x.log");
    }

    #[test]
    fn adb_status_message_uses_camel_case_fields() {
        let payload = serde_json::to_value(ServerMessage::AdbStatus {
            available: true,
            mode: AdbStatusMode::Bundled,
            path: Some("libs/linux/adb".to_string()),
            message: "ADB: using bundled libs/linux/adb".to_string(),
        })
        .expect("adb_status serializes");

        assert_eq!(payload["type"], "adb_status");
        assert_eq!(payload["available"], true);
        assert_eq!(payload["mode"], "bundled");
        assert_eq!(payload["path"], "libs/linux/adb");
        assert_eq!(payload["message"], "ADB: using bundled libs/linux/adb");
    }

    #[test]
    fn device_list_message_uses_manager_mock_fallback_device() {
        let manager = crate::device_manager::DeviceManager::mock_fallback(
            "ADB: no online devices, using mock device",
        );
        let payload =
            serde_json::to_value(device_list_message(&manager)).expect("device_list serializes");

        assert_eq!(payload["type"], "device_list");
        assert_eq!(payload["devices"][0]["deviceId"], MOCK_DEVICE_ID);
        assert_eq!(payload["devices"][0]["deviceName"], "Mock Device");
        assert_eq!(payload["devices"][0]["connected"], true);
        assert_eq!(payload["devices"][0]["source"], "mock");
    }

    #[test]
    fn startup_recorder_status_targets_manager_devices() {
        let mock_manager =
            DeviceManager::mock_fallback("ADB: no online devices, using mock device");
        let adb_manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![crate::adb::AdbDevice {
                serial: "emulator-5554".to_string(),
                display_name: "Pixel 8".to_string(),
            }],
        );

        assert_eq!(
            startup_recorder_status_device_ids(&mock_manager),
            vec![MOCK_DEVICE_ID]
        );
        assert_eq!(
            startup_recorder_status_device_ids(&adb_manager),
            vec!["emulator-5554".to_string()]
        );
    }

    #[test]
    fn refresh_device_state_targets_manager_devices() {
        let mock_manager =
            DeviceManager::mock_fallback("ADB: no online devices, using mock device");
        let adb_manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![crate::adb::AdbDevice {
                serial: "emulator-5554".to_string(),
                display_name: "Pixel 8".to_string(),
            }],
        );

        assert_eq!(
            refresh_device_state_device_ids(&mock_manager),
            vec![MOCK_DEVICE_ID]
        );
        assert_eq!(
            refresh_device_state_device_ids(&adb_manager),
            vec!["emulator-5554".to_string()]
        );
    }

    #[test]
    fn refresh_device_state_messages_reset_visible_device_state() {
        let manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![crate::adb::AdbDevice {
                serial: "emulator-5554".to_string(),
                display_name: "Pixel 8".to_string(),
            }],
        );

        let messages = refresh_device_state_messages(&manager, "emulator-5554")
            .expect("refresh state messages");

        assert_eq!(messages.len(), 3);
        assert!(matches!(
            &messages[0],
            ServerMessage::RecorderStatus { device_id, .. } if device_id == "emulator-5554"
        ));
        assert!(matches!(
            &messages[1],
            ServerMessage::LogSnapshot { device_id, logs } if device_id == "emulator-5554" && logs.is_empty()
        ));
        assert!(matches!(
            &messages[2],
            ServerMessage::Statistics { device_id, .. } if device_id == "emulator-5554"
        ));
    }

    #[test]
    fn adb_status_message_uses_manager_mock_fallback_status() {
        let manager = crate::device_manager::DeviceManager::mock_fallback(
            "ADB: no online devices, using mock device",
        );
        let payload = serde_json::to_value(adb_status_message(manager.adb_status()))
            .expect("manager adb_status serializes");

        assert_eq!(payload["type"], "adb_status");
        assert_eq!(payload["available"], false);
        assert_eq!(payload["mode"], "mock_fallback");
        assert!(payload["path"].is_null());
        assert_eq!(
            payload["message"],
            "ADB: no online devices, using mock device"
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
    fn pending_adb_log_messages_include_stats_for_hidden_entries() {
        let messages = pending_adb_log_messages(
            "emulator-5554",
            LogEntry {
                seq: 1,
                timestamp: 0,
                date: "07-04".to_string(),
                time: "12:00:00.000".to_string(),
                pid: 1234,
                tid: 5678,
                level: crate::log_entry::LogLevel::Error,
                tag: "ActivityManager".to_string(),
                message: "Hidden crash".to_string(),
                package_name: Some("com.example".to_string()),
                foreground: None,
                background: None,
                hidden: true,
                bookmarked: false,
            },
        );

        assert_eq!(messages.len(), 2);
        assert!(matches!(messages[0], PendingAdbLogMessage::RecorderStatus));
        assert!(matches!(messages[1], PendingAdbLogMessage::Statistics));
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
