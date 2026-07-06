use crate::adb::{list_devices, logcat_command, resolve_adb_path, AdbDevice};
use crate::device::{DeviceContext, DeviceSnapshot};
use crate::filter::FilterQuery;
use crate::log_entry::{DeviceInfo, DeviceSource, LogEntry};
use crate::recorder::{Recorder, RecorderConfig};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;

#[allow(dead_code)]
pub const MOCK_DEVICE_ID: &str = "mock-device";
#[allow(dead_code)]
pub const MOCK_DEVICE_NAME: &str = "Mock Device";

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdbStatusMode {
    Bundled,
    MockFallback,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbStatus {
    pub available: bool,
    pub mode: AdbStatusMode,
    pub path: Option<String>,
    pub message: String,
}

#[allow(dead_code)]
pub struct DeviceManager {
    adb_status: AdbStatus,
    devices: Vec<DeviceInfo>,
    contexts: HashMap<String, DeviceContext>,
    logcat_children: HashMap<String, Child>,
    log_receiver: Option<mpsc::UnboundedReceiver<(String, String)>>,
}

#[allow(dead_code)]
impl DeviceManager {
    pub fn mock_fallback(message: impl Into<String>) -> Self {
        Self::mock_fallback_with_log_root(message, PathBuf::from("logs"))
    }

    fn mock_fallback_with_log_root(message: impl Into<String>, log_root: PathBuf) -> Self {
        let recorder = Recorder::new(RecorderConfig {
            enabled: true,
            root: log_root,
            device_name: MOCK_DEVICE_ID.to_string(),
        });
        let context = DeviceContext::new(
            MOCK_DEVICE_ID.to_string(),
            MOCK_DEVICE_NAME.to_string(),
            1_000_000,
            recorder,
        );
        let mut contexts = HashMap::new();
        contexts.insert(MOCK_DEVICE_ID.to_string(), context);

        Self {
            adb_status: AdbStatus {
                available: false,
                mode: AdbStatusMode::MockFallback,
                path: None,
                message: message.into(),
            },
            devices: vec![DeviceInfo {
                device_id: MOCK_DEVICE_ID.to_string(),
                device_name: MOCK_DEVICE_NAME.to_string(),
                connected: true,
                source: DeviceSource::Mock,
            }],
            contexts,
            logcat_children: HashMap::new(),
            log_receiver: None,
        }
    }

    pub fn from_adb_devices(path: String, adb_devices: Vec<AdbDevice>) -> Self {
        Self::from_adb_devices_with_log_root(path, adb_devices, PathBuf::from("logs"))
    }

    pub fn from_scan_result(
        path: Option<String>,
        devices: Vec<AdbDevice>,
        error: Option<String>,
    ) -> Self {
        if let Some(error) = error {
            return Self::mock_fallback(format!("ADB: {error}, using mock device"));
        }

        if devices.is_empty() {
            return Self::mock_fallback("ADB: no online devices, using mock device");
        }

        Self::from_adb_devices(
            path.unwrap_or_else(|| "libs/<platform>/adb".to_string()),
            devices,
        )
    }

    pub async fn start(project_root: &std::path::Path) -> Self {
        let adb_path = resolve_adb_path(project_root).adb;
        let adb_path_string = adb_path.display().to_string();

        if !adb_path.exists() {
            return Self::mock_fallback(format!(
                "ADB: missing {adb_path_string}, using mock device"
            ));
        }

        match list_devices(&adb_path).await {
            Ok(devices) if devices.is_empty() => {
                Self::mock_fallback("ADB: no online devices, using mock device")
            }
            Ok(devices) => {
                let mut manager = Self::from_adb_devices(adb_path_string, devices);
                manager.start_logcat_processes(&adb_path).await;
                manager
            }
            Err(error) => Self::mock_fallback(format!("ADB: {error}, using mock device")),
        }
    }

    async fn start_logcat_processes(&mut self, adb_path: &std::path::Path) {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.log_receiver = Some(receiver);

        let adb_device_ids = self
            .devices
            .iter()
            .filter(|device| device.source == DeviceSource::Adb)
            .map(|device| device.device_id.clone())
            .collect::<Vec<_>>();

        for device_id in adb_device_ids {
            let mut command = logcat_command(adb_path, &device_id);
            command.stdout(Stdio::piped()).stderr(Stdio::null());

            match command.spawn() {
                Ok(mut child) => {
                    if let Some(stdout) = child.stdout.take() {
                        let serial = device_id.clone();
                        let sender = sender.clone();
                        tokio::spawn(async move {
                            let mut lines = BufReader::new(stdout).lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                let _ = sender.send((serial.clone(), line));
                            }
                        });
                    }
                    self.logcat_children.insert(device_id, child);
                }
                Err(error) => {
                    self.switch_to_mock_fallback(format!(
                        "ADB: failed to start logcat: {error}, using mock device"
                    ));
                    return;
                }
            }
        }
    }

    fn switch_to_mock_fallback(&mut self, message: impl Into<String>) {
        self.stop_logcat_children();
        *self = Self::mock_fallback(message);
    }

    fn stop_logcat_children(&mut self) {
        for child in self.logcat_children.values_mut() {
            let _ = child.start_kill();
        }
        self.logcat_children.clear();
    }

    fn from_adb_devices_with_log_root(
        path: String,
        adb_devices: Vec<AdbDevice>,
        log_root: PathBuf,
    ) -> Self {
        let count = adb_devices.len();
        let mut contexts = HashMap::new();
        let devices = adb_devices
            .into_iter()
            .map(|device| {
                let device_id = device.serial;
                let device_name = device.display_name;
                let recorder = Recorder::new(RecorderConfig {
                    enabled: true,
                    root: log_root.clone(),
                    device_name: device_id.clone(),
                });
                let context =
                    DeviceContext::new(device_id.clone(), device_name.clone(), 1_000_000, recorder);
                contexts.insert(device_id.clone(), context);

                DeviceInfo {
                    device_id,
                    device_name,
                    connected: true,
                    source: DeviceSource::Adb,
                }
            })
            .collect();

        Self {
            adb_status: AdbStatus {
                available: true,
                mode: AdbStatusMode::Bundled,
                path: Some(path),
                message: format!("ADB: {count} device(s) connected"),
            },
            devices,
            contexts,
            logcat_children: HashMap::new(),
            log_receiver: None,
        }
    }

    pub fn adb_status(&self) -> &AdbStatus {
        &self.adb_status
    }

    pub fn device_list(&self) -> &[DeviceInfo] {
        &self.devices
    }

    pub fn has_device(&self, device_id: &str) -> bool {
        self.contexts.contains_key(device_id)
    }

    pub fn is_mock_fallback(&self) -> bool {
        self.devices
            .iter()
            .any(|device| device.source == DeviceSource::Mock)
    }

    pub fn ingest_mock_line(&mut self, raw_line: &str) -> Option<LogEntry> {
        self.contexts
            .get_mut(MOCK_DEVICE_ID)
            .and_then(|context| context.ingest_line(raw_line))
    }

    pub fn drain_pending_logs(&mut self) -> Vec<(String, LogEntry)> {
        let mut entries = Vec::new();
        let Some(receiver) = self.log_receiver.as_mut() else {
            return entries;
        };

        while let Ok((device_id, line)) = receiver.try_recv() {
            if let Some(context) = self.contexts.get_mut(&device_id) {
                if let Some(entry) = context.ingest_line(&line) {
                    entries.push((device_id, entry));
                }
            }
        }

        entries
    }

    pub fn set_filter(&mut self, device_id: &str, query: &str) -> anyhow::Result<()> {
        let context = self
            .contexts
            .get_mut(device_id)
            .ok_or_else(|| anyhow::anyhow!("unknown device: {device_id}"))?;
        context.set_filter(FilterQuery::parse(query));
        Ok(())
    }

    pub fn latest_visible_snapshot(
        &self,
        device_id: &str,
        limit: usize,
    ) -> anyhow::Result<DeviceSnapshot> {
        let context = self
            .contexts
            .get(device_id)
            .ok_or_else(|| anyhow::anyhow!("unknown device: {device_id}"))?;
        Ok(context.latest_visible_snapshot(limit))
    }

    pub fn search_visible_sequences(
        &self,
        device_id: &str,
        query: &str,
    ) -> anyhow::Result<Vec<u64>> {
        let context = self
            .contexts
            .get(device_id)
            .ok_or_else(|| anyhow::anyhow!("unknown device: {device_id}"))?;
        Ok(context.search_visible_sequences(query))
    }
}

impl Drop for DeviceManager {
    fn drop(&mut self) {
        self.stop_logcat_children();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn builds_mock_fallback_when_adb_is_missing() {
        let manager =
            DeviceManager::mock_fallback("ADB: missing libs/linux/adb, using mock device");

        assert_eq!(manager.adb_status().available, false);
        assert_eq!(manager.adb_status().mode, AdbStatusMode::MockFallback);
        assert_eq!(manager.device_list().len(), 1);
        assert_eq!(manager.device_list()[0].device_id, MOCK_DEVICE_ID);
        assert_eq!(manager.device_list()[0].source, DeviceSource::Mock);
    }

    #[test]
    fn no_adb_devices_uses_mock_fallback_status() {
        let manager =
            DeviceManager::from_scan_result(Some("libs/linux/adb".to_string()), Vec::new(), None);

        assert_eq!(manager.adb_status().available, false);
        assert_eq!(manager.device_list()[0].source, DeviceSource::Mock);
        assert!(manager.adb_status().message.contains("no online devices"));
    }

    #[test]
    fn adb_scan_error_uses_mock_fallback_status() {
        let manager = DeviceManager::from_scan_result(
            Some("libs/linux/adb".to_string()),
            Vec::new(),
            Some("permission denied".to_string()),
        );

        assert_eq!(manager.adb_status().available, false);
        assert!(manager.adb_status().message.contains("permission denied"));
    }

    #[test]
    fn builds_adb_devices_for_online_devices() {
        let devices = vec![
            AdbDevice {
                serial: "emulator-5554".to_string(),
                display_name: "Pixel 8".to_string(),
            },
            AdbDevice {
                serial: "R58N123ABC".to_string(),
                display_name: "Galaxy S23".to_string(),
            },
        ];

        let manager = DeviceManager::from_adb_devices("libs/linux/adb".to_string(), devices);

        assert_eq!(manager.adb_status().available, true);
        assert_eq!(manager.adb_status().mode, AdbStatusMode::Bundled);
        assert_eq!(manager.adb_status().message, "ADB: 2 device(s) connected");
        assert_eq!(manager.device_list().len(), 2);
        assert_eq!(manager.device_list()[0].source, DeviceSource::Adb);
        assert_eq!(manager.device_list()[1].device_name, "Galaxy S23");
    }

    #[tokio::test]
    async fn logcat_spawn_failure_replaces_adb_state_with_mock_fallback() {
        let mut manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![AdbDevice {
                serial: "emulator-5554".to_string(),
                display_name: "Pixel 8".to_string(),
            }],
        );

        manager
            .start_logcat_processes(&PathBuf::from("/definitely/missing/adb"))
            .await;

        assert_eq!(manager.adb_status().available, false);
        assert_eq!(manager.adb_status().mode, AdbStatusMode::MockFallback);
        assert!(manager
            .adb_status()
            .message
            .contains("failed to start logcat"));
        assert_eq!(manager.device_list().len(), 1);
        assert_eq!(manager.device_list()[0].source, DeviceSource::Mock);
        assert!(manager.is_mock_fallback());
        assert!(manager.has_device(MOCK_DEVICE_ID));
        assert!(!manager.has_device("emulator-5554"));
    }

    #[test]
    fn identifies_mock_fallback_mode() {
        let mock_manager =
            DeviceManager::mock_fallback("ADB: no online devices, using mock device");
        let adb_manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![AdbDevice {
                serial: "emulator-5554".to_string(),
                display_name: "Pixel 8".to_string(),
            }],
        );

        assert!(mock_manager.is_mock_fallback());
        assert!(!adb_manager.is_mock_fallback());
    }

    #[test]
    fn mock_manager_ingests_and_searches_logs() {
        let dir = tempdir().expect("tempdir");
        let mut manager = DeviceManager::mock_fallback_with_log_root(
            "ADB: no online devices, using mock device",
            dir.path().to_path_buf(),
        );

        manager.ingest_mock_line("07-04 12:34:56.789  1234  5678 I ActivityManager: Mock log line");
        manager
            .set_filter(MOCK_DEVICE_ID, "ActivityManager")
            .expect("filter should apply");

        let snapshot = manager
            .latest_visible_snapshot(MOCK_DEVICE_ID, 500)
            .expect("snapshot");
        assert_eq!(snapshot.logs.len(), 1);
        assert_eq!(
            manager
                .search_visible_sequences(MOCK_DEVICE_ID, "Mock")
                .expect("search"),
            vec![1]
        );
    }

    #[test]
    fn unknown_device_returns_error() {
        let mut manager = DeviceManager::mock_fallback("ADB: no online devices, using mock device");

        let error = manager
            .set_filter("missing", "level:error")
            .expect_err("unknown device should error");
        assert!(error.to_string().contains("unknown device: missing"));
    }
}
