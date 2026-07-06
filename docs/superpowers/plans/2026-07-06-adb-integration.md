# ADB Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add real Android device support through bundled ADB binaries in `libs/<platform>/`, while preserving Mock Device fallback.

**Architecture:** Split ADB integration into three backend layers: `adb.rs` for bundled ADB path/process parsing, `device_manager.rs` for real/mock device lifecycle, and `websocket.rs` for protocol transport. The renderer gains ADB status display and a manual Refresh Devices action.

**Tech Stack:** Rust 2021, Tokio, Axum WebSocket, serde, Electron main/preload, React, Zustand, Playwright.

---

## File Structure

- Modify `engine/src/adb.rs`: resolve `libs/` ADB path, parse `adb devices -l`, represent ADB device/status, and build logcat commands.
- Create `engine/src/device_manager.rs`: manage real ADB devices, mock fallback, per-device `DeviceContext`, logcat child processes, snapshots, searches, and refresh.
- Modify `engine/src/main.rs`: register the new `device_manager` module.
- Modify `engine/src/log_entry.rs`: add `source: DeviceSource` to `DeviceInfo`.
- Modify `engine/src/websocket.rs`: add `refresh_devices` and `adb_status`, replace direct mock ticking with `DeviceManager`.
- Modify `src/renderer/types/protocol.ts`: add `DeviceSource`, `AdbStatus`, `refresh_devices`, and `adb_status`.
- Modify `src/renderer/state/appStore.ts`: store ADB status and handle `adb_status`.
- Modify `src/renderer/components/StatusBar.tsx`: render ADB status text.
- Modify `src/renderer/components/DeviceTabs.tsx`: show source-aware device tabs.
- Modify `src/renderer/App.tsx`: wire a Refresh Devices button to the engine client.
- Modify `tests/e2e/app.spec.ts`: extend the smoke test for ADB status and refresh behavior.

---

### Task 1: Implement bundled ADB path resolution and devices parsing

**Files:**
- Modify: `engine/src/adb.rs`

- [ ] **Step 1: Replace `adb.rs` tests with bundled path and parser tests**

Use this complete test module at the bottom of `engine/src/adb.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn resolves_platform_adb_under_libs() {
        let paths = resolve_adb_path(Path::new("/app"));
        let rendered = paths.adb.display().to_string();

        assert!(rendered.contains("libs"));
        assert!(rendered.contains("adb"));
        assert!(!rendered.contains("tools"));
    }

    #[test]
    fn parses_online_devices_with_model_name() {
        let output = "List of devices attached\n\
emulator-5554 device product:sdk_gphone64_x86_64 model:Pixel_8 device:emu64 transport_id:1\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].display_name, "Pixel 8");
    }

    #[test]
    fn parses_multiple_online_devices() {
        let output = "List of devices attached\n\
emulator-5554 device model:Pixel_8\n\
R58N123ABC device model:Galaxy_S23\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[1].serial, "R58N123ABC");
    }

    #[test]
    fn ignores_non_online_devices() {
        let output = "List of devices attached\n\
emulator-5554 offline model:Pixel_8\n\
R58N123ABC unauthorized model:Galaxy_S23\n\
ZX1 recovery model:Recovery_Device\n\
OK1 device model:Online_Device\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "OK1");
        assert_eq!(devices[0].display_name, "Online Device");
    }

    #[test]
    fn uses_serial_when_model_is_missing() {
        let output = "List of devices attached\nabc123 device usb:1-1\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].display_name, "abc123");
    }
}
```

- [ ] **Step 2: Run parser tests and verify failure**

Run:

```bash
cargo test -p als-engine adb::tests -- --nocapture
```

Expected: FAIL because `parse_devices_output`, `AdbDevice`, and the `libs/` path behavior do not exist yet.

- [ ] **Step 3: Replace `engine/src/adb.rs` with the implementation**

Use this file content:

```rust
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbPaths {
    pub adb: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbDevice {
    pub serial: String,
    pub display_name: String,
}

pub fn resolve_adb_path(project_root: &Path) -> AdbPaths {
    let relative = if cfg!(target_os = "windows") {
        PathBuf::from("libs/windows/adb.exe")
    } else if cfg!(target_os = "macos") {
        PathBuf::from("libs/macos/adb")
    } else {
        PathBuf::from("libs/linux/adb")
    };

    AdbPaths { adb: project_root.join(relative) }
}

pub fn parse_devices_output(output: &str) -> Vec<AdbDevice> {
    output
        .lines()
        .skip(1)
        .filter_map(parse_device_line)
        .collect()
}

fn parse_device_line(line: &str) -> Option<AdbDevice> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let serial = parts.next()?.to_string();
    let state = parts.next()?;
    if state != "device" {
        return None;
    }

    let model = parts
        .find_map(|part| part.strip_prefix("model:"))
        .map(|model| model.replace('_', " "))
        .unwrap_or_else(|| serial.clone());

    Some(AdbDevice { serial, display_name: model })
}

pub async fn list_devices(adb_path: &Path) -> anyhow::Result<Vec<AdbDevice>> {
    let output = Command::new(adb_path).arg("devices").arg("-l").output().await?;
    if !output.status.success() {
        anyhow::bail!("adb devices -l exited with {}", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_devices_output(&stdout))
}

pub fn logcat_command(adb_path: &Path, serial: &str) -> Command {
    let mut command = Command::new(adb_path);
    command.arg("-s").arg(serial).arg("logcat").arg("-v").arg("threadtime");
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn resolves_platform_adb_under_libs() {
        let paths = resolve_adb_path(Path::new("/app"));
        let rendered = paths.adb.display().to_string();

        assert!(rendered.contains("libs"));
        assert!(rendered.contains("adb"));
        assert!(!rendered.contains("tools"));
    }

    #[test]
    fn parses_online_devices_with_model_name() {
        let output = "List of devices attached\n\
emulator-5554 device product:sdk_gphone64_x86_64 model:Pixel_8 device:emu64 transport_id:1\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].display_name, "Pixel 8");
    }

    #[test]
    fn parses_multiple_online_devices() {
        let output = "List of devices attached\n\
emulator-5554 device model:Pixel_8\n\
R58N123ABC device model:Galaxy_S23\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[1].serial, "R58N123ABC");
    }

    #[test]
    fn ignores_non_online_devices() {
        let output = "List of devices attached\n\
emulator-5554 offline model:Pixel_8\n\
R58N123ABC unauthorized model:Galaxy_S23\n\
ZX1 recovery model:Recovery_Device\n\
OK1 device model:Online_Device\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "OK1");
        assert_eq!(devices[0].display_name, "Online Device");
    }

    #[test]
    fn uses_serial_when_model_is_missing() {
        let output = "List of devices attached\nabc123 device usb:1-1\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].display_name, "abc123");
    }
}
```

- [ ] **Step 4: Run tests and verify pass**

Run:

```bash
cargo test -p als-engine adb::tests -- --nocapture
```

Expected: PASS, 5 tests.

- [ ] **Step 5: Run full engine tests**

Run:

```bash
cargo test -p als-engine
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add engine/src/adb.rs
git commit -m "feat: parse bundled adb devices"
```

---

### Task 2: Add device source and ADB protocol types

**Files:**
- Modify: `engine/src/log_entry.rs`
- Modify: `engine/src/websocket.rs`
- Modify: `src/renderer/types/protocol.ts`

- [ ] **Step 1: Add failing Rust protocol tests**

In `engine/src/websocket.rs`, add these tests inside `mod tests`:

```rust
#[test]
fn refresh_devices_message_deserializes() {
    let message = serde_json::from_value::<ClientMessage>(json!({
        "type": "refresh_devices"
    }))
    .expect("refresh_devices should deserialize");

    assert!(matches!(message, ClientMessage::RefreshDevices));
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
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p als-engine websocket::tests::refresh_devices_message_deserializes websocket::tests::adb_status_message_uses_camel_case_fields
```

Expected: FAIL because `RefreshDevices`, `AdbStatus`, and `AdbStatusMode` do not exist.

- [ ] **Step 3: Add Rust protocol types**

In `engine/src/log_entry.rs`, replace `DeviceInfo` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceSource {
    Adb,
    Mock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub connected: bool,
    pub source: DeviceSource,
}
```

In `engine/src/websocket.rs`, add this enum near `ServerMessage`:

```rust
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdbStatusMode {
    Bundled,
    MockFallback,
}
```

Add the client message variant:

```rust
RefreshDevices,
```

Add the server message variant:

```rust
AdbStatus {
    available: bool,
    mode: AdbStatusMode,
    path: Option<String>,
    message: String,
},
```

Update `device_list_message()` mock `DeviceInfo` construction to include:

```rust
source: crate::log_entry::DeviceSource::Mock,
```

- [ ] **Step 4: Update TypeScript protocol types**

In `src/renderer/types/protocol.ts`, add:

```ts
export type DeviceSource = 'adb' | 'mock';

export interface AdbStatus {
  available: boolean;
  mode: 'bundled' | 'mock_fallback';
  path: string | null;
  message: string;
}
```

Change `DeviceInfo` to:

```ts
export interface DeviceInfo {
  deviceId: string;
  deviceName: string;
  connected: boolean;
  source: DeviceSource;
}
```

Add client message:

```ts
| { type: 'refresh_devices' }
```

Add server message:

```ts
| { type: 'adb_status'; available: boolean; mode: 'bundled' | 'mock_fallback'; path: string | null; message: string }
```

- [ ] **Step 5: Run Rust and TypeScript verification**

Run:

```bash
cargo test -p als-engine websocket::tests::refresh_devices_message_deserializes websocket::tests::adb_status_message_uses_camel_case_fields
npm run build
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add engine/src/log_entry.rs engine/src/websocket.rs src/renderer/types/protocol.ts
git commit -m "feat: add adb protocol types"
```

---

### Task 3: Add `DeviceManager` with mock fallback state

**Files:**
- Create: `engine/src/device_manager.rs`
- Modify: `engine/src/main.rs`
- Modify: `engine/src/websocket.rs`

- [ ] **Step 1: Create `device_manager.rs` with tests first**

Create `engine/src/device_manager.rs` with this test-focused skeleton:

```rust
use crate::adb::AdbDevice;
use crate::log_entry::{DeviceInfo, DeviceSource};

const MOCK_DEVICE_ID: &str = "mock-device";
const MOCK_DEVICE_NAME: &str = "Mock Device";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdbStatusMode {
    Bundled,
    MockFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbStatus {
    pub available: bool,
    pub mode: AdbStatusMode,
    pub path: Option<String>,
    pub message: String,
}

pub struct DeviceManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_mock_fallback_when_adb_is_missing() {
        let manager = DeviceManager::mock_fallback("ADB: missing libs/linux/adb, using mock device");

        assert_eq!(manager.adb_status().available, false);
        assert_eq!(manager.adb_status().mode, AdbStatusMode::MockFallback);
        assert_eq!(manager.device_list().len(), 1);
        assert_eq!(manager.device_list()[0].device_id, MOCK_DEVICE_ID);
        assert_eq!(manager.device_list()[0].source, DeviceSource::Mock);
    }

    #[test]
    fn builds_adb_devices_for_online_devices() {
        let devices = vec![
            AdbDevice { serial: "emulator-5554".to_string(), display_name: "Pixel 8".to_string() },
            AdbDevice { serial: "R58N123ABC".to_string(), display_name: "Galaxy S23".to_string() },
        ];

        let manager = DeviceManager::from_adb_devices("libs/linux/adb".to_string(), devices);

        assert_eq!(manager.adb_status().available, true);
        assert_eq!(manager.adb_status().mode, AdbStatusMode::Bundled);
        assert_eq!(manager.device_list().len(), 2);
        assert_eq!(manager.device_list()[0].source, DeviceSource::Adb);
        assert_eq!(manager.device_list()[1].device_name, "Galaxy S23");
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p als-engine device_manager::tests
```

Expected: FAIL until module is registered and methods are implemented.

- [ ] **Step 3: Register module**

In `engine/src/main.rs`, add:

```rust
mod device_manager;
```

- [ ] **Step 4: Implement minimal `DeviceManager` state**

Replace the `pub struct DeviceManager;` line and add impl:

```rust
pub struct DeviceManager {
    adb_status: AdbStatus,
    devices: Vec<DeviceInfo>,
}

impl DeviceManager {
    pub fn mock_fallback(message: impl Into<String>) -> Self {
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
        }
    }

    pub fn from_adb_devices(path: String, adb_devices: Vec<AdbDevice>) -> Self {
        let count = adb_devices.len();
        Self {
            adb_status: AdbStatus {
                available: true,
                mode: AdbStatusMode::Bundled,
                path: Some(path),
                message: format!("ADB: {count} device{} connected", if count == 1 { "" } else { "s" }),
            },
            devices: adb_devices
                .into_iter()
                .map(|device| DeviceInfo {
                    device_id: device.serial,
                    device_name: device.display_name,
                    connected: true,
                    source: DeviceSource::Adb,
                })
                .collect(),
        }
    }

    pub fn adb_status(&self) -> &AdbStatus {
        &self.adb_status
    }

    pub fn device_list(&self) -> &[DeviceInfo] {
        &self.devices
    }
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p als-engine device_manager::tests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add engine/src/device_manager.rs engine/src/main.rs
git commit -m "feat: add device manager fallback state"
```

---

### Task 4: Move mock device runtime into `DeviceManager`

**Files:**
- Modify: `engine/src/device_manager.rs`
- Modify: `engine/src/websocket.rs`

- [ ] **Step 1: Add tests for mock snapshots and filter/search**

Add to `engine/src/device_manager.rs` tests:

```rust
#[test]
fn mock_manager_ingests_and_searches_logs() {
    let mut manager = DeviceManager::mock_fallback("ADB: no online devices, using mock device");

    manager.ingest_mock_line("07-04 12:34:56.789  1234  5678 I ActivityManager: Mock log line");
    manager.set_filter(MOCK_DEVICE_ID, "ActivityManager").expect("filter should apply");

    let snapshot = manager.latest_visible_snapshot(MOCK_DEVICE_ID, 500).expect("snapshot");
    assert_eq!(snapshot.logs.len(), 1);
    assert_eq!(manager.search_visible_sequences(MOCK_DEVICE_ID, "Mock").expect("search"), vec![1]);
}

#[test]
fn unknown_device_returns_error() {
    let mut manager = DeviceManager::mock_fallback("ADB: no online devices, using mock device");

    let error = manager.set_filter("missing", "level:error").expect_err("unknown device should error");
    assert!(error.to_string().contains("unknown device: missing"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p als-engine device_manager::tests
```

Expected: FAIL because runtime methods do not exist.

- [ ] **Step 3: Extend `DeviceManager` with contexts**

Add imports:

```rust
use crate::device::{DeviceContext, DeviceSnapshot};
use crate::filter::FilterQuery;
use crate::recorder::{Recorder, RecorderConfig};
use std::collections::HashMap;
use std::path::PathBuf;
```

Change struct:

```rust
pub struct DeviceManager {
    adb_status: AdbStatus,
    devices: Vec<DeviceInfo>,
    contexts: HashMap<String, DeviceContext>,
}
```

Update constructors to create contexts. For mock fallback:

```rust
let recorder = Recorder::new(RecorderConfig {
    enabled: true,
    root: PathBuf::from("logs"),
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
```

For ADB devices, create one `DeviceContext` per device using `device.serial` as `device_id` and recorder `device_name`.

- [ ] **Step 4: Add runtime methods**

Add to `impl DeviceManager`:

```rust
pub fn ingest_mock_line(&mut self, raw_line: &str) {
    if let Some(context) = self.contexts.get_mut(MOCK_DEVICE_ID) {
        let _ = context.ingest_line(raw_line);
    }
}

pub fn set_filter(&mut self, device_id: &str, query: &str) -> anyhow::Result<()> {
    let context = self
        .contexts
        .get_mut(device_id)
        .ok_or_else(|| anyhow::anyhow!("unknown device: {device_id}"))?;
    context.set_filter(FilterQuery::parse(query));
    Ok(())
}

pub fn latest_visible_snapshot(&self, device_id: &str, limit: usize) -> anyhow::Result<DeviceSnapshot> {
    let context = self
        .contexts
        .get(device_id)
        .ok_or_else(|| anyhow::anyhow!("unknown device: {device_id}"))?;
    Ok(context.latest_visible_snapshot(limit))
}

pub fn search_visible_sequences(&self, device_id: &str, query: &str) -> anyhow::Result<Vec<u64>> {
    let context = self
        .contexts
        .get(device_id)
        .ok_or_else(|| anyhow::anyhow!("unknown device: {device_id}"))?;
    Ok(context.search_visible_sequences(query))
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p als-engine device_manager::tests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add engine/src/device_manager.rs
git commit -m "feat: manage mock device runtime"
```

---

### Task 5: Wire `DeviceManager` into WebSocket for mock fallback

**Files:**
- Modify: `engine/src/websocket.rs`

- [ ] **Step 1: Update WebSocket tests for ADB status serialization imports**

If Task 2 defined `AdbStatusMode` in `device_manager.rs`, import it in `websocket.rs`:

```rust
use crate::device_manager::{AdbStatus, AdbStatusMode, DeviceManager};
```

Update the `adb_status_message_uses_camel_case_fields` test to serialize the WebSocket `ServerMessage::AdbStatus` with `AdbStatusMode::Bundled` from `device_manager`.

- [ ] **Step 2: Replace direct `DeviceContext` use with manager in socket state**

In `handle_socket`, replace:

```rust
let mut device = mock_device_context();
```

with:

```rust
let mut manager = DeviceManager::mock_fallback("ADB: no online devices, using mock device");
```

Send initial messages:

```rust
if !send_server_message(&mut sender, &device_list_message(&manager)).await {
    return;
}
if !send_adb_status(&mut sender, manager.adb_status()).await {
    return;
}
if !send_recorder_status(&mut sender, &manager, "mock-device").await {
    return;
}
```

- [ ] **Step 3: Replace mock tick**

Use this mock tick body:

```rust
async fn send_mock_tick(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &mut DeviceManager,
) -> bool {
    manager.ingest_mock_line(MOCK_LOG_LINE);
    let snapshot = match manager.latest_visible_snapshot(MOCK_DEVICE_ID, 1) {
        Ok(snapshot) => snapshot,
        Err(error) => return send_error(sender, error.to_string()).await,
    };

    if let Some(entry) = snapshot.logs.last().cloned() {
        let message = ServerMessage::NewLogs {
            device_id: MOCK_DEVICE_ID.to_string(),
            logs: vec![entry],
        };
        if !send_server_message(sender, &message).await {
            return false;
        }
    }

    send_recorder_status(sender, manager, MOCK_DEVICE_ID).await
        && send_statistics(sender, manager, MOCK_DEVICE_ID).await
}
```

- [ ] **Step 4: Update client handlers**

Change handler signature to:

```rust
async fn handle_client_text(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &mut DeviceManager,
    text: &str,
) -> bool
```

For `SetFilter`, call:

```rust
if let Err(error) = manager.set_filter(&device_id, &query) {
    return send_error(sender, error.to_string()).await;
}
send_visible_snapshot(sender, manager, &device_id).await && send_statistics(sender, manager, &device_id).await
```

For `SetSearch`, call:

```rust
let matches = match manager.search_visible_sequences(&device_id, &query) {
    Ok(matches) => matches,
    Err(error) => return send_error(sender, error.to_string()).await,
};
let message = ServerMessage::SearchResults { device_id, matches };
send_server_message(sender, &message).await
```

For `GetStatistics`, call `send_statistics(sender, manager, &device_id).await`.

For connect/disconnect, validate by checking `manager.latest_visible_snapshot(&device_id, 1).is_ok()`; if not, send an unknown-device error.

- [ ] **Step 5: Add helper functions**

Add:

```rust
async fn send_error(sender: &mut SplitSink<WebSocket, Message>, message: String) -> bool {
    send_server_message(sender, &ServerMessage::Error { message }).await
}

async fn send_adb_status(sender: &mut SplitSink<WebSocket, Message>, status: &AdbStatus) -> bool {
    let message = ServerMessage::AdbStatus {
        available: status.available,
        mode: status.mode,
        path: status.path.clone(),
        message: status.message.clone(),
    };
    send_server_message(sender, &message).await
}
```

Update `device_list_message`, `send_visible_snapshot`, `send_statistics`, and `send_recorder_status` to accept `DeviceManager` and `device_id`.

- [ ] **Step 6: Remove direct mock context helper**

Delete `mock_device_context()` from `websocket.rs`. `DeviceManager` now owns mock creation.

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test -p als-engine websocket::tests device_manager::tests
npm run build
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add engine/src/websocket.rs engine/src/device_manager.rs
git commit -m "feat: route websocket through device manager"
```

---

### Task 6: Add real ADB scan and logcat process startup

**Files:**
- Modify: `engine/src/device_manager.rs`
- Modify: `engine/src/websocket.rs`

- [ ] **Step 1: Add tests for scan decisions without spawning real ADB**

Add tests to `device_manager.rs`:

```rust
#[test]
fn no_adb_devices_uses_mock_fallback_status() {
    let manager = DeviceManager::from_scan_result(
        Some("libs/linux/adb".to_string()),
        Vec::new(),
        None,
    );

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
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p als-engine device_manager::tests::no_adb_devices_uses_mock_fallback_status device_manager::tests::adb_scan_error_uses_mock_fallback_status
```

Expected: FAIL because `from_scan_result` does not exist.

- [ ] **Step 3: Implement `from_scan_result`**

Add:

```rust
pub fn from_scan_result(path: Option<String>, devices: Vec<AdbDevice>, error: Option<String>) -> Self {
    if let Some(error) = error {
        return Self::mock_fallback(format!("ADB: {error}, using mock device"));
    }

    if devices.is_empty() {
        return Self::mock_fallback("ADB: no online devices, using mock device");
    }

    Self::from_adb_devices(path.unwrap_or_else(|| "libs/<platform>/adb".to_string()), devices)
}
```

- [ ] **Step 4: Add async startup constructor**

Add imports:

```rust
use crate::adb::{list_devices, logcat_command, resolve_adb_path};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;
```

Add struct fields:

```rust
logcat_children: HashMap<String, Child>,
log_receiver: Option<mpsc::UnboundedReceiver<(String, String)>>,
```

Add constructor:

```rust
pub async fn start(project_root: &std::path::Path) -> Self {
    let adb_path = resolve_adb_path(project_root).adb;
    let adb_path_string = adb_path.display().to_string();

    if !adb_path.exists() {
        return Self::mock_fallback(format!("ADB: missing {adb_path_string}, using mock device"));
    }

    match list_devices(&adb_path).await {
        Ok(devices) if devices.is_empty() => Self::mock_fallback("ADB: no online devices, using mock device"),
        Ok(devices) => {
            let mut manager = Self::from_adb_devices(adb_path_string, devices);
            manager.start_logcat_processes(&adb_path).await;
            manager
        }
        Err(error) => Self::mock_fallback(format!("ADB: {error}, using mock device")),
    }
}
```

Add process start method:

```rust
async fn start_logcat_processes(&mut self, adb_path: &std::path::Path) {
    let (sender, receiver) = mpsc::unbounded_channel();
    self.log_receiver = Some(receiver);

    for device in self.devices.iter().filter(|device| device.source == DeviceSource::Adb) {
        let mut command = logcat_command(adb_path, &device.device_id);
        command.stdout(Stdio::piped()).stderr(Stdio::null());

        match command.spawn() {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    let serial = device.device_id.clone();
                    let sender = sender.clone();
                    tokio::spawn(async move {
                        let mut lines = BufReader::new(stdout).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = sender.send((serial.clone(), line));
                        }
                    });
                }
                self.logcat_children.insert(device.device_id.clone(), child);
            }
            Err(error) => {
                self.adb_status.available = false;
                self.adb_status.mode = AdbStatusMode::MockFallback;
                self.adb_status.message = format!("ADB: failed to start logcat: {error}, using mock device");
            }
        }
    }
}
```

Add draining method:

```rust
pub fn drain_pending_logs(&mut self) -> Vec<(String, crate::log_entry::LogEntry)> {
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
```

- [ ] **Step 5: Implement cleanup**

Add:

```rust
impl Drop for DeviceManager {
    fn drop(&mut self) {
        for child in self.logcat_children.values_mut() {
            let _ = child.start_kill();
        }
    }
}
```

- [ ] **Step 6: Wire WebSocket startup to async manager**

Change `handle_socket` initialization to:

```rust
let project_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
let mut manager = DeviceManager::start(&project_root).await;
```

Change ticker handling to:

```rust
if manager.is_mock_fallback() {
    if !send_mock_tick(&mut sender, &mut manager).await {
        break;
    }
} else if !send_pending_adb_logs(&mut sender, &mut manager).await {
    break;
}
```

Add `is_mock_fallback()` to `DeviceManager`:

```rust
pub fn is_mock_fallback(&self) -> bool {
    self.devices.iter().any(|device| device.source == DeviceSource::Mock)
}
```

Add WebSocket helper:

```rust
async fn send_pending_adb_logs(
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &mut DeviceManager,
) -> bool {
    for (device_id, entry) in manager.drain_pending_logs() {
        if entry.hidden {
            continue;
        }
        let message = ServerMessage::NewLogs { device_id: device_id.clone(), logs: vec![entry] };
        if !send_server_message(sender, &message).await {
            return false;
        }
        if !send_recorder_status(sender, manager, &device_id).await {
            return false;
        }
        if !send_statistics(sender, manager, &device_id).await {
            return false;
        }
    }
    true
}
```

- [ ] **Step 7: Run tests and build**

Run:

```bash
cargo test -p als-engine
npm run build
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add engine/src/device_manager.rs engine/src/websocket.rs
git commit -m "feat: start bundled adb logcat"
```

---

### Task 7: Add Refresh Devices support

**Files:**
- Modify: `engine/src/device_manager.rs`
- Modify: `engine/src/websocket.rs`

- [ ] **Step 1: Add refresh test**

Add to `device_manager.rs` tests:

```rust
#[test]
fn refresh_replaces_device_state() {
    let mut manager = DeviceManager::from_adb_devices(
        "libs/linux/adb".to_string(),
        vec![AdbDevice { serial: "old".to_string(), display_name: "Old".to_string() }],
    );

    manager.replace_with_scan_result(
        Some("libs/linux/adb".to_string()),
        vec![AdbDevice { serial: "new".to_string(), display_name: "New".to_string() }],
        None,
    );

    assert_eq!(manager.device_list().len(), 1);
    assert_eq!(manager.device_list()[0].device_id, "new");
}
```

- [ ] **Step 2: Implement replacement method**

Add:

```rust
pub fn replace_with_scan_result(
    &mut self,
    path: Option<String>,
    devices: Vec<AdbDevice>,
    error: Option<String>,
) {
    for child in self.logcat_children.values_mut() {
        let _ = child.start_kill();
    }
    *self = Self::from_scan_result(path, devices, error);
}
```

- [ ] **Step 3: Implement async refresh**

Add:

```rust
pub async fn refresh(&mut self, project_root: &std::path::Path) {
    for child in self.logcat_children.values_mut() {
        let _ = child.start_kill();
    }

    let adb_path = resolve_adb_path(project_root).adb;
    let adb_path_string = adb_path.display().to_string();
    if !adb_path.exists() {
        *self = Self::mock_fallback(format!("ADB: missing {adb_path_string}, using mock device"));
        return;
    }

    match list_devices(&adb_path).await {
        Ok(devices) if devices.is_empty() => {
            *self = Self::mock_fallback("ADB: no online devices, using mock device");
        }
        Ok(devices) => {
            *self = Self::from_adb_devices(adb_path_string, devices);
            self.start_logcat_processes(&adb_path).await;
        }
        Err(error) => {
            *self = Self::mock_fallback(format!("ADB: {error}, using mock device"));
        }
    }
}
```

- [ ] **Step 4: Wire WebSocket `RefreshDevices`**

In `handle_client_text`, add:

```rust
Ok(ClientMessage::RefreshDevices) => {
    let project_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    manager.refresh(&project_root).await;
    send_adb_status(sender, manager.adb_status()).await
        && send_server_message(sender, &device_list_message(manager)).await
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p als-engine device_manager::tests::refresh_replaces_device_state websocket::tests::refresh_devices_message_deserializes
```

Expected: PASS.

- [ ] **Step 6: Run full checks**

Run:

```bash
cargo test -p als-engine
npm run build
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add engine/src/device_manager.rs engine/src/websocket.rs
git commit -m "feat: refresh adb devices"
```

---

### Task 8: Add renderer ADB status and refresh UI

**Files:**
- Modify: `src/renderer/state/appStore.ts`
- Modify: `src/renderer/components/StatusBar.tsx`
- Modify: `src/renderer/components/DeviceTabs.tsx`
- Modify: `src/renderer/App.tsx`

- [ ] **Step 1: Update store state**

In `src/renderer/state/appStore.ts`, import `AdbStatus` and add state:

```ts
adbStatus: AdbStatus | null;
```

Initial value:

```ts
adbStatus: null,
```

Handle server message:

```ts
case 'adb_status':
  set({ adbStatus: message });
  break;
```

- [ ] **Step 2: Update StatusBar props and rendering**

Replace `StatusBarProps` with:

```ts
import type { AdbStatus } from '../types/protocol';

interface StatusBarProps {
  connected: boolean;
  adbStatus: AdbStatus | null;
  recorderPath: string | null;
  visibleLogCount: number;
  warning: string | null;
}
```

Update function signature and add status segment:

```tsx
export function StatusBar({ connected, adbStatus, recorderPath, visibleLogCount, warning }: StatusBarProps) {
  return (
    <footer className="status-bar">
      <span className={connected ? 'status status--connected' : 'status status--disconnected'}>
        {connected ? 'connected' : 'disconnected'}
      </span>
      <span>{adbStatus?.message ?? 'ADB: pending'}</span>
      <span>Recorder: {recorderPath ?? 'pending'}</span>
      <span>{visibleLogCount} visible logs</span>
      {warning ? <strong>{warning}</strong> : null}
    </footer>
  );
}
```

- [ ] **Step 3: Update DeviceTabs source display**

Inside each button in `DeviceTabs.tsx`, after device ID add:

```tsx
<span className="device-tab__source">{device.source}</span>
```

- [ ] **Step 4: Add Refresh Devices button in App**

In `App.tsx`, add:

```ts
const adbStatus = useAppStore((state) => state.adbStatus);
```

Add handler:

```ts
const handleRefreshDevices = useCallback(() => {
  clientRef.current?.send({ type: 'refresh_devices' });
}, []);
```

Render button near query controls:

```tsx
<section className="query-region" aria-label="Query controls">
  <QueryBar value={filterQuery} onChange={handleFilterChange} />
  <button className="refresh-devices" type="button" onClick={handleRefreshDevices}>
    Refresh Devices
  </button>
</section>
```

Pass status:

```tsx
<StatusBar
  connected={connected}
  adbStatus={adbStatus}
  recorderPath={recorderPath}
  visibleLogCount={logs.length}
  warning={recorderWarning}
/>
```

- [ ] **Step 5: Add minimal CSS**

In `src/renderer/styles.css`, add:

```css
.refresh-devices {
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--surface-raised);
  color: var(--text-primary);
  padding: 0 14px;
  min-height: 40px;
  cursor: pointer;
}

.device-tab__source {
  color: var(--text-muted);
  font-size: 11px;
  text-transform: uppercase;
}
```

Use existing CSS variable names. If `--surface-raised` or `--text-primary` does not exist, use the closest existing variables in `styles.css` and keep the button visually consistent.

- [ ] **Step 6: Run build**

Run:

```bash
npm run build
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/renderer/state/appStore.ts src/renderer/components/StatusBar.tsx src/renderer/components/DeviceTabs.tsx src/renderer/App.tsx src/renderer/styles.css
git commit -m "feat: show adb status and refresh action"
```

---

### Task 9: Extend E2E smoke for ADB status and refresh message

**Files:**
- Modify: `tests/e2e/app.spec.ts`

- [ ] **Step 1: Replace Playwright test with WebSocket fake server coverage**

Use this test file:

```ts
import { expect, test } from '@playwright/test';
import { WebSocketServer } from 'ws';

test('renders shell, adb status, and sends refresh_devices', async ({ page }) => {
  const messages: string[] = [];
  const server = new WebSocketServer({ port: 0 });
  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('expected websocket server address');
  }

  server.on('connection', (socket) => {
    socket.on('message', (message) => messages.push(String(message)));
    socket.send(JSON.stringify({
      type: 'device_list',
      devices: [{ deviceId: 'mock-device', deviceName: 'Mock Device', connected: true, source: 'mock' }],
    }));
    socket.send(JSON.stringify({
      type: 'adb_status',
      available: false,
      mode: 'mock_fallback',
      path: null,
      message: 'ADB: no online devices, using mock device',
    }));
  });

  await page.addInitScript((port) => {
    window.als = {
      version: '0.1.0',
      getEngineUrl: async () => `ws://127.0.0.1:${port}/ws`,
    };
  }, address.port);

  await page.goto('http://127.0.0.1:5173');

  await expect(page.getByRole('heading', { name: 'Android Logcat Studio' })).toBeVisible();
  await expect(page.getByText('ADB: no online devices, using mock device')).toBeVisible();
  await expect(page.getByText('mock')).toBeVisible();

  await page.getByRole('button', { name: 'Refresh Devices' }).click();
  await expect.poll(() => messages.some((message) => message.includes('refresh_devices'))).toBe(true);

  server.close();
});
```

- [ ] **Step 2: Add `ws` dev dependency if TypeScript cannot resolve it**

Run:

```bash
npm run test:e2e
```

If it fails with `Cannot find module 'ws'`, install types/package:

```bash
npm install --save-dev ws @types/ws
```

Then commit the resulting `package.json` and `package-lock.json` changes with this task.

- [ ] **Step 3: Run E2E**

Run:

```bash
npm run test:e2e
```

Expected: PASS, one Playwright test.

- [ ] **Step 4: Run full checks**

Run:

```bash
cargo test -p als-engine
npm run build
npm run test:e2e
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/e2e/app.spec.ts package.json package-lock.json
git commit -m "test: cover adb status refresh flow"
```

---

### Task 10: Manual runtime verification and acceptance update

**Files:**
- Modify: `docs/superpowers/plans/2026-07-04-android-logcat-studio-mvp-acceptance.md`
- Create: `docs/CHANGELOG-2026-07-06-adb.md`

- [ ] **Step 1: Verify missing ADB fallback**

Temporarily move the current platform ADB binary out of the expected path if it exists. Example on Linux:

```bash
mv libs/linux/adb libs/linux/adb.disabled
```

Run:

```bash
npm run build
```

Launch the app if local Electron sandbox allows it:

```bash
npm run dev:electron
```

Expected UI: Mock Device appears and StatusBar shows a message like `ADB: missing ... using mock device`.

Restore the binary if moved:

```bash
mv libs/linux/adb.disabled libs/linux/adb
```

If Electron cannot launch because of local Linux `chrome-sandbox` permissions, record that runtime UI verification is blocked by local sandbox configuration and continue with WebSocket-level verification.

- [ ] **Step 2: Verify WebSocket-level fallback without launching Electron**

Run the engine with a fixed verification token:

```bash
ALS_ENGINE_TOKEN=verify-token cargo run -p als-engine
```

Connect with a WebSocket client to:

```text
ws://127.0.0.1:<port>/ws?token=verify-token
```

Expected first messages include `device_list` with Mock Device and `adb_status` with `mock_fallback` when no usable ADB is available.

- [ ] **Step 3: Verify real ADB if a device is available**

With `libs/<platform>/adb` present and at least one online Android device connected, launch the engine and connect to the WebSocket URL.

Expected:

- `device_list` contains every online device with `source: 'adb'`.
- `adb_status.message` reports connected device count.
- Logcat lines arrive as `new_logs`.
- `set_filter` still sends `log_snapshot` and updated statistics.

If no real device is available, state that real-device manual verification was not run.

- [ ] **Step 4: Update acceptance checklist**

In `docs/superpowers/plans/2026-07-04-android-logcat-studio-mvp-acceptance.md`, add these lines after the Mock Device line:

```markdown
- [x] Engine resolves bundled ADB from `libs/<platform>/adb` or `libs/<platform>/adb.exe`.
- [x] ADB unavailable/no-device state falls back to Mock Device with a StatusBar message.
- [ ] Real connected devices stream `adb logcat -v threadtime` into the UI. `【未验证】` Requires a connected Android device during manual verification.
```

Only check the real-device line if Step 3 was run with a real device.

- [ ] **Step 5: Add changelog**

Create `docs/CHANGELOG-2026-07-06-adb.md`:

```markdown
# Changelog 2026-07-06 ADB Integration

- Added bundled ADB path resolution under `libs/linux`, `libs/macos`, and `libs/windows`.
- Added startup device scanning with Mock Device fallback when ADB is unavailable or no online devices are connected.
- Added multi-device logcat process management through `DeviceManager`.
- Added `refresh_devices` and `adb_status` WebSocket protocol messages.
- Added renderer ADB status display and Refresh Devices action.
- Preserved existing filtering, search, recording, and smoke-test flows.
```

- [ ] **Step 6: Run final checks**

Run:

```bash
cargo test -p als-engine
npm run build
npm run test:e2e
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/plans/2026-07-04-android-logcat-studio-mvp-acceptance.md docs/CHANGELOG-2026-07-06-adb.md
git commit -m "docs: update adb integration acceptance"
```

---

## Self-Review

### Spec coverage

- Bundled ADB layout: Task 1 resolves `libs/linux/adb`, `libs/macos/adb`, and `libs/windows/adb.exe`.
- ADB parsing: Task 1 parses online devices and ignores offline/unauthorized/recovery states.
- Device lifecycle: Tasks 3, 4, 6, and 7 introduce `DeviceManager`, mock fallback, logcat processes, and refresh.
- WebSocket protocol: Tasks 2, 5, and 7 add `refresh_devices`, `adb_status`, and manager-backed message flow.
- Renderer UI: Task 8 adds ADB status, device source display, and Refresh Devices.
- Tests and verification: Tasks 1 through 9 add automated coverage; Task 10 covers manual/runtime checks and docs.

### Placeholder scan

This plan avoids placeholder markers. Each task lists exact files, commands, expected outcomes, and concrete code snippets.

### Type consistency

Rust `DeviceSource` serializes as `adb | mock`, matching TypeScript `DeviceSource`. Rust `AdbStatusMode` serializes as `bundled | mock_fallback`, matching TypeScript `AdbStatus.mode`. Client message `RefreshDevices` maps to JSON `refresh_devices` under existing snake_case serde rules.
