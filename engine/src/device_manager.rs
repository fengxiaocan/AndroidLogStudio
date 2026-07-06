use crate::adb::AdbDevice;
use crate::device::{DeviceContext, DeviceSnapshot};
use crate::filter::FilterQuery;
use crate::log_entry::{DeviceInfo, DeviceSource, LogEntry};
use crate::recorder::{Recorder, RecorderConfig};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

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
        }
    }

    pub fn from_adb_devices(path: String, adb_devices: Vec<AdbDevice>) -> Self {
        Self::from_adb_devices_with_log_root(path, adb_devices, PathBuf::from("logs"))
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
        self.adb_status.mode == AdbStatusMode::MockFallback && self.has_device(MOCK_DEVICE_ID)
    }

    pub fn ingest_mock_line(&mut self, raw_line: &str) -> Option<LogEntry> {
        self.contexts
            .get_mut(MOCK_DEVICE_ID)
            .and_then(|context| context.ingest_line(raw_line))
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
