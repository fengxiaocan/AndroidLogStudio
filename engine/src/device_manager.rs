use crate::adb::{list_devices, logcat_command, resolve_adb_path, AdbDevice};
use crate::device::{DeviceContext, DeviceSnapshot, ExportMode};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedExportResult {
    pub path: PathBuf,
    pub line_count: usize,
    pub mode: ExportMode,
}

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

    pub fn replace_with_scan_result(
        &mut self,
        path: Option<String>,
        devices: Vec<AdbDevice>,
        error: Option<String>,
    ) {
        self.merge_scan_result(path, devices, error);
    }

    /// Merge a scan into existing state without wiping buffers.
    /// - error: soft-disconnect all ADB devices; update adb_status; keep contexts
    /// - success: add new devices, soft-disconnect missing ADB serials, update names/path
    /// Does not start logcat children (caller starts them for online devices).
    pub fn merge_scan_result(
        &mut self,
        path: Option<String>,
        devices: Vec<AdbDevice>,
        error: Option<String>,
    ) {
        if let Some(error) = error {
            let has_adb = self
                .devices
                .iter()
                .any(|device| device.source == DeviceSource::Adb);
            if has_adb {
                for device in self.devices.iter_mut() {
                    if device.source == DeviceSource::Adb {
                        device.connected = false;
                    }
                }
                self.stop_logcat_children();
                self.adb_status = AdbStatus {
                    available: path.is_some(),
                    mode: AdbStatusMode::Bundled,
                    path,
                    message: format!("ADB: {error}"),
                };
            } else {
                self.switch_to_mock_fallback(format!("ADB: {error}, using mock device"));
            }
            return;
        }

        // Empty scan with existing ADB devices → soft-disconnect all ADB; do not force mock.
        if devices.is_empty() {
            let has_adb = self
                .devices
                .iter()
                .any(|device| device.source == DeviceSource::Adb);
            if has_adb {
                for device in self.devices.iter_mut() {
                    if device.source == DeviceSource::Adb {
                        device.connected = false;
                    }
                }
                self.stop_logcat_children();
                let path_string = path.unwrap_or_else(|| "libs/<platform>/adb".to_string());
                self.adb_status = AdbStatus {
                    available: true,
                    mode: AdbStatusMode::Bundled,
                    path: Some(path_string),
                    message: "ADB: no online devices".to_string(),
                };
            } else {
                self.switch_to_mock_fallback("ADB: no online devices, using mock device");
            }
            return;
        }

        // If we are currently mock-only, replace with ADB set (first real attach).
        if self.is_mock_fallback() {
            let path_string = path.unwrap_or_else(|| "libs/<platform>/adb".to_string());
            *self = Self::from_adb_devices(path_string, devices);
            return;
        }

        let path_string = path.unwrap_or_else(|| "libs/<platform>/adb".to_string());
        let log_root = PathBuf::from("logs");
        let scanned: HashMap<String, AdbDevice> = devices
            .into_iter()
            .map(|device| (device.serial.clone(), device))
            .collect();

        // Soft-disconnect ADB devices missing from scan
        let existing_ids: Vec<String> = self
            .devices
            .iter()
            .filter(|device| device.source == DeviceSource::Adb)
            .map(|device| device.device_id.clone())
            .collect();
        for id in &existing_ids {
            if !scanned.contains_key(id) {
                self.mark_disconnected(id);
            }
        }

        // Update or add scanned devices
        for (serial, adb_device) in &scanned {
            if let Some(device) = self
                .devices
                .iter_mut()
                .find(|device| device.device_id == *serial)
            {
                device.connected = true;
                device.device_name = adb_device.display_name.clone();
                device.source = DeviceSource::Adb;
            } else {
                let recorder = Recorder::new(RecorderConfig {
                    enabled: true,
                    root: log_root.clone(),
                    device_name: serial.clone(),
                });
                let context = DeviceContext::new(
                    serial.clone(),
                    adb_device.display_name.clone(),
                    1_000_000,
                    recorder,
                );
                self.contexts.insert(serial.clone(), context);
                self.devices.push(DeviceInfo {
                    device_id: serial.clone(),
                    device_name: adb_device.display_name.clone(),
                    connected: true,
                    source: DeviceSource::Adb,
                });
            }
        }

        self.adb_status = AdbStatus {
            available: true,
            mode: AdbStatusMode::Bundled,
            path: Some(path_string),
            message: String::new(),
        };
        self.refresh_adb_status_message();
    }

    pub fn refresh_adb_status_message(&mut self) {
        if self.is_mock_fallback() {
            return;
        }
        let online = self.devices.iter().filter(|device| device.connected).count();
        let disconnected = self
            .devices
            .iter()
            .filter(|device| !device.connected)
            .count();
        self.adb_status.message = if disconnected == 0 {
            format!("ADB: {online} device(s) connected")
        } else {
            format!("ADB: {online} online, {disconnected} disconnected")
        };
    }

    /// Stub to satisfy websocket after merge of pid cache work.
    /// Full integration (per-context cache + adb ps) can be added later.
    pub async fn refresh_pid_caches_if_needed(&mut self) {
        // TODO
    }

    pub async fn refresh(&mut self, project_root: &std::path::Path) {
        let adb_path = resolve_adb_path(project_root).adb;
        let adb_path_string = adb_path.display().to_string();
        if !adb_path.exists() {
            self.merge_scan_result(
                Some(adb_path_string.clone()),
                Vec::new(),
                Some(format!("missing {adb_path_string}")),
            );
            return;
        }

        match list_devices(&adb_path).await {
            Ok(devices) => {
                self.merge_scan_result(Some(adb_path_string), devices, None);
                self.ensure_logcat_for_connected(&adb_path).await;
            }
            Err(error) => {
                self.merge_scan_result(
                    Some(adb_path_string),
                    Vec::new(),
                    Some(error.to_string()),
                );
            }
        }
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
                    self.switch_to_mock_fallback_async(format!(
                        "ADB: failed to start logcat: {error}, using mock device"
                    ))
                    .await;
                    return;
                }
            }
        }
    }

    /// Restart logcat children for every currently connected ADB device.
    /// Buffers/contexts are preserved; only the process pipes are recreated.
    async fn ensure_logcat_for_connected(&mut self, adb_path: &std::path::Path) {
        self.stop_logcat_children_async().await;

        let online: Vec<String> = self
            .devices
            .iter()
            .filter(|device| device.source == DeviceSource::Adb && device.connected)
            .map(|device| device.device_id.clone())
            .collect();

        if online.is_empty() {
            self.log_receiver = None;
            return;
        }

        let (sender, receiver) = mpsc::unbounded_channel();
        self.log_receiver = Some(receiver);

        for device_id in online {
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
                    // Soft-fail one device: mark disconnected, continue others.
                    self.mark_disconnected(&device_id);
                    self.adb_status.message =
                        format!("ADB: failed to start logcat for {device_id}: {error}");
                }
            }
        }
    }

    fn switch_to_mock_fallback(&mut self, message: impl Into<String>) {
        self.stop_logcat_children();
        *self = Self::mock_fallback(message);
    }

    async fn switch_to_mock_fallback_async(&mut self, message: impl Into<String>) {
        self.stop_logcat_children_async().await;
        *self = Self::mock_fallback(message);
    }

    fn stop_logcat_children(&mut self) {
        for child in self.logcat_children.values_mut() {
            let _ = child.start_kill();
        }
        self.logcat_children.clear();
    }

    async fn stop_logcat_children_async(&mut self) {
        let children = std::mem::take(&mut self.logcat_children);
        for (_, mut child) in children {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
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

    /// Soft-disconnect: stop logcat child if any, set connected=false, keep context.
    pub fn mark_disconnected(&mut self, device_id: &str) -> bool {
        let Some(device) = self
            .devices
            .iter_mut()
            .find(|device| device.device_id == device_id)
        else {
            return false;
        };
        if !device.connected {
            if let Some(mut child) = self.logcat_children.remove(device_id) {
                let _ = child.start_kill();
            }
            return true;
        }
        device.connected = false;
        if let Some(mut child) = self.logcat_children.remove(device_id) {
            let _ = child.start_kill();
        }
        true
    }

    /// Poll logcat children for exit. Soft-disconnect any that have exited.
    /// Returns true if the device list changed.
    pub async fn poll_logcat_exits(&mut self) -> bool {
        let mut exited = Vec::new();
        for (device_id, child) in self.logcat_children.iter_mut() {
            match child.try_wait() {
                Ok(Some(_)) | Err(_) => exited.push(device_id.clone()),
                Ok(None) => {}
            }
        }
        let mut dirty = false;
        for device_id in exited {
            self.logcat_children.remove(&device_id);
            if self.mark_disconnected(&device_id) {
                dirty = true;
            }
        }
        if dirty {
            self.refresh_adb_status_message();
        }
        dirty
    }

    pub fn is_connected(&self, device_id: &str) -> bool {
        self.devices
            .iter()
            .find(|device| device.device_id == device_id)
            .map(|device| device.connected)
            .unwrap_or(false)
    }

    pub fn remove_device(&mut self, device_id: &str) -> anyhow::Result<()> {
        let connected = self
            .devices
            .iter()
            .find(|device| device.device_id == device_id)
            .map(|device| device.connected);
        match connected {
            None => anyhow::bail!("unknown device: {device_id}"),
            Some(true) => anyhow::bail!("device still connected: {device_id}"),
            Some(false) => {
                self.contexts.remove(device_id);
                self.logcat_children.remove(device_id);
                self.devices.retain(|device| device.device_id != device_id);
                Ok(())
            }
        }
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

    pub fn export_logs(
        &self,
        device_id: &str,
        mode: ExportMode,
    ) -> anyhow::Result<ManagedExportResult> {
        let context = self
            .contexts
            .get(device_id)
            .ok_or_else(|| anyhow::anyhow!("unknown device: {device_id}"))?;

        let exports_dir = PathBuf::from("logs").join("exports");
        std::fs::create_dir_all(&exports_dir)?;

        let safe_id: String = device_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let mode_label = match mode {
            ExportMode::All => "all",
            ExportMode::Filtered => "filtered",
        };
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let file_name = format!("{safe_id}-{mode_label}-{millis}.log");
        let path = exports_dir.join(file_name);

        let device_result = context.export_logs(mode, &path)?;
        let absolute = std::fs::canonicalize(&path).unwrap_or(path);

        Ok(ManagedExportResult {
            path: absolute,
            line_count: device_result.line_count,
            mode,
        })
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
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn remove_device_rejects_connected_and_unknown() {
        let mut manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![AdbDevice {
                serial: "serial-a".to_string(),
                display_name: "Phone A".to_string(),
            }],
        );
        let err = manager
            .remove_device("serial-a")
            .expect_err("connected device cannot be removed");
        assert!(err.to_string().contains("still connected"));

        let err = manager
            .remove_device("missing")
            .expect_err("unknown device");
        assert!(err.to_string().contains("unknown device"));
    }

    #[test]
    fn remove_device_drops_disconnected_context() {
        let mut manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![
                AdbDevice {
                    serial: "serial-a".to_string(),
                    display_name: "Phone A".to_string(),
                },
                AdbDevice {
                    serial: "serial-b".to_string(),
                    display_name: "Phone B".to_string(),
                },
            ],
        );
        assert!(manager.mark_disconnected("serial-a"));
        manager.remove_device("serial-a").expect("remove ok");
        assert!(!manager.has_device("serial-a"));
        assert_eq!(manager.device_list().len(), 1);
        assert_eq!(manager.device_list()[0].device_id, "serial-b");
    }

    #[test]
    fn mark_disconnected_keeps_context_and_sets_flag() {
        let mut manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![AdbDevice {
                serial: "serial-a".to_string(),
                display_name: "Phone A".to_string(),
            }],
        );
        let line = "07-04 12:34:56.789  1234  5678 I Tag: keep me";
        {
            let ctx = manager.contexts.get_mut("serial-a").expect("context");
            assert!(ctx.ingest_line(line).is_some());
        }

        assert!(manager.mark_disconnected("serial-a"));
        let device = manager
            .device_list()
            .iter()
            .find(|d| d.device_id == "serial-a")
            .expect("device remains listed");
        assert!(!device.connected);
        assert!(manager.has_device("serial-a"));
        let snap = manager
            .latest_visible_snapshot("serial-a", 100)
            .expect("snapshot still works");
        assert_eq!(snap.logs.len(), 1);
    }

    #[tokio::test]
    async fn poll_logcat_exits_marks_device_disconnected() {
        let mut manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![AdbDevice {
                serial: "serial-a".to_string(),
                display_name: "Phone A".to_string(),
            }],
        );
        let child = tokio::process::Command::new("true")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("true should spawn");
        manager
            .logcat_children
            .insert("serial-a".to_string(), child);

        tokio::time::sleep(Duration::from_millis(50)).await;

        let dirty = manager.poll_logcat_exits().await;
        assert!(dirty);
        let device = manager
            .device_list()
            .iter()
            .find(|d| d.device_id == "serial-a")
            .expect("listed");
        assert!(!device.connected);
        assert!(!manager.logcat_children.contains_key("serial-a"));
    }

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

    #[tokio::test]
    async fn async_stop_logcat_children_waits_for_process_exit() {
        let mut manager = DeviceManager::mock_fallback("ADB: no online devices, using mock device");
        let child = tokio::process::Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sleep should spawn");
        let pid = child.id().expect("child should have pid");
        manager
            .logcat_children
            .insert(MOCK_DEVICE_ID.to_string(), child);

        manager.stop_logcat_children_async().await;

        assert!(manager.logcat_children.is_empty());
        let status = std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stderr(Stdio::null())
            .status()
            .expect("kill -0 should run");
        assert!(!status.success(), "child process should be reaped");
    }

    #[test]
    fn merge_scan_soft_disconnects_missing_and_keeps_buffer() {
        let log_root = tempdir().expect("tempdir");
        let mut manager = DeviceManager::from_adb_devices_with_log_root(
            "libs/linux/adb".to_string(),
            vec![
                AdbDevice {
                    serial: "keep".to_string(),
                    display_name: "Keep".to_string(),
                },
                AdbDevice {
                    serial: "gone".to_string(),
                    display_name: "Gone".to_string(),
                },
            ],
            log_root.path().to_path_buf(),
        );
        {
            let ctx = manager.contexts.get_mut("gone").unwrap();
            ctx.ingest_line("07-04 12:34:56.789  1  1 I Tag: history");
        }

        manager.merge_scan_result(
            Some("libs/linux/adb".to_string()),
            vec![AdbDevice {
                serial: "keep".to_string(),
                display_name: "Keep".to_string(),
            }],
            None,
        );

        assert_eq!(manager.device_list().len(), 2);
        let gone = manager
            .device_list()
            .iter()
            .find(|d| d.device_id == "gone")
            .unwrap();
        assert!(!gone.connected);
        let snap = manager.latest_visible_snapshot("gone", 10).unwrap();
        assert_eq!(snap.logs.len(), 1);

        let keep = manager
            .device_list()
            .iter()
            .find(|d| d.device_id == "keep")
            .unwrap();
        assert!(keep.connected);
    }

    #[test]
    fn merge_scan_adds_new_device_without_dropping_existing() {
        let log_root = tempdir().expect("tempdir");
        let mut manager = DeviceManager::from_adb_devices_with_log_root(
            "libs/linux/adb".to_string(),
            vec![AdbDevice {
                serial: "old".to_string(),
                display_name: "Old".to_string(),
            }],
            log_root.path().to_path_buf(),
        );
        {
            let ctx = manager.contexts.get_mut("old").unwrap();
            ctx.ingest_line("07-04 12:34:56.789  1  1 I Tag: old-log");
        }

        manager.merge_scan_result(
            Some("libs/linux/adb".to_string()),
            vec![
                AdbDevice {
                    serial: "old".to_string(),
                    display_name: "Old".to_string(),
                },
                AdbDevice {
                    serial: "new".to_string(),
                    display_name: "New".to_string(),
                },
            ],
            None,
        );

        assert_eq!(manager.device_list().len(), 2);
        assert!(manager.has_device("old"));
        assert!(manager.has_device("new"));
        let snap = manager.latest_visible_snapshot("old", 10).unwrap();
        assert_eq!(snap.logs.len(), 1);
    }

    #[test]
    fn merge_scan_error_soft_disconnects_all_adb_devices() {
        let mut manager = DeviceManager::from_adb_devices(
            "libs/linux/adb".to_string(),
            vec![AdbDevice {
                serial: "serial-a".to_string(),
                display_name: "Phone A".to_string(),
            }],
        );
        manager.merge_scan_result(
            Some("libs/linux/adb".to_string()),
            Vec::new(),
            Some("permission denied".to_string()),
        );
        assert!(!manager.device_list()[0].connected);
        assert!(manager.adb_status().message.contains("permission denied"));
        assert!(manager.has_device("serial-a"));
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

    #[test]
    fn manager_export_logs_unknown_device_errors() {
        use crate::device::ExportMode;

        let manager = DeviceManager::mock_fallback("test");
        let err = manager
            .export_logs("missing", ExportMode::All)
            .expect_err("unknown");
        assert!(err.to_string().contains("unknown device"));
    }

    #[test]
    fn manager_export_logs_writes_under_exports_dir() {
        use crate::device::ExportMode;

        let mut manager = DeviceManager::mock_fallback("test");
        assert!(manager
            .ingest_mock_line("07-16 12:00:00.000  1  1 I Tag: line")
            .is_some());

        let result = manager
            .export_logs(MOCK_DEVICE_ID, ExportMode::All)
            .expect("export");
        assert_eq!(result.line_count, 1);
        assert!(result.path.exists());
        assert!(
            result.path.to_string_lossy().contains("exports"),
            "path should be under exports: {:?}",
            result.path
        );
        let text = std::fs::read_to_string(&result.path).unwrap();
        assert!(text.contains("Tag: line"));
        // cleanup temp file
        let _ = std::fs::remove_file(&result.path);
    }
}
