# Device Disconnect & Per-Device Log Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Soft-disconnect devices when logcat dies or Refresh loses them, fill the log list from engine snapshots on device switch, and allow removing only disconnected devices without wiping other devices’ buffers.

**Architecture:** Engine remains source of truth (`DeviceContext` per device). Add lifecycle APIs on `DeviceManager` (poll child exits, merge refresh, remove disconnected). Wire `connect_device` to push `log_snapshot` and add `remove_device`. Frontend sends `connect_device` on switch, keeps logs when only `connected` flips, and exposes a remove control for disconnected actives.

**Tech Stack:** Rust (`als-engine`, tokio process), Axum WebSocket JSON protocol, Electron React + Zustand, Vitest, cargo test.

**Spec:** `docs/superpowers/specs/2026-07-15-device-disconnect-and-log-isolation-design.md`

---

## File map

| File | Responsibility |
|---|---|
| `engine/src/device_manager.rs` | Soft disconnect, poll logcat exits, merge refresh, `remove_device`, connected flags |
| `engine/src/websocket.rs` | `ConnectDevice` → snapshot; `RemoveDevice` handler; tick polls exits → `device_list` |
| `src/renderer/types/protocol.ts` | Add `remove_device` client message |
| `src/renderer/state/appStore.ts` | `device_list` keeps logs on connected-only flip |
| `src/renderer/state/appStore.test.ts` | Store behavior tests |
| `src/renderer/App.tsx` | Switch → `connect_device`; remove button → `remove_device` |
| `src/renderer/components/DeviceSelect.tsx` | Disconnected label |
| `src/renderer/settings/i18n.ts` | `deviceDisconnected`, `removeDevice` |
| `src/renderer/styles.css` | Optional remove-button / disconnected option styling if needed |

---

### Task 1: Soft-disconnect helpers + poll logcat exits

**Files:**
- Modify: `engine/src/device_manager.rs`
- Test: same file `#[cfg(test)]` module

- [ ] **Step 1: Write failing tests for soft disconnect and poll**

Add inside `mod tests` in `engine/src/device_manager.rs`:

```rust
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
    // Short-lived process so try_wait eventually sees exit.
    let child = tokio::process::Command::new("true")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("true should spawn");
    manager
        .logcat_children
        .insert("serial-a".to_string(), child);

    // Yield so the process can exit.
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
```

Add imports in the test module if missing:

```rust
use std::time::Duration;
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p als-engine mark_disconnected_keeps_context -- --nocapture
cargo test -p als-engine poll_logcat_exits_marks_device -- --nocapture
```

Expected: FAIL (methods not found).

- [ ] **Step 3: Implement `mark_disconnected` and `poll_logcat_exits`**

In `impl DeviceManager` (public API section near `has_device`):

```rust
/// Soft-disconnect: stop logcat child if any, set connected=false, keep context.
/// Returns true if the device existed and was updated (or already disconnected).
pub fn mark_disconnected(&mut self, device_id: &str) -> bool {
    let Some(device) = self
        .devices
        .iter_mut()
        .find(|device| device.device_id == device_id)
    else {
        return false;
    };
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
            Ok(Some(_status)) => exited.push(device_id.clone()),
            Ok(None) => {}
            Err(_) => exited.push(device_id.clone()),
        }
    }
    if exited.is_empty() {
        return false;
    }
    for device_id in exited {
        let _ = self.logcat_children.remove(&device_id);
        self.mark_disconnected(&device_id);
    }
    true
}

pub fn is_connected(&self, device_id: &str) -> bool {
    self.devices
        .iter()
        .find(|device| device.device_id == device_id)
        .map(|device| device.connected)
        .unwrap_or(false)
}
```

Note: `mark_disconnected` already sets `connected = false` and removes the child; in `poll_logcat_exits` remove child first then call `mark_disconnected` (idempotent on missing child). Adjust so `mark_disconnected` does not double-kill:

Prefer this shape for `poll_logcat_exits`:

```rust
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
    dirty
}
```

And `mark_disconnected` only toggles flag + kills child if still present:

```rust
pub fn mark_disconnected(&mut self, device_id: &str) -> bool {
    let Some(device) = self
        .devices
        .iter_mut()
        .find(|device| device.device_id == device_id)
    else {
        return false;
    };
    if !device.connected {
        // Still ensure no orphan child.
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
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p als-engine mark_disconnected_keeps_context poll_logcat_exits_marks_device -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add engine/src/device_manager.rs
git commit -m "feat(engine): soft-disconnect on logcat exit"
```

---

### Task 2: `remove_device` on DeviceManager

**Files:**
- Modify: `engine/src/device_manager.rs`

- [ ] **Step 1: Write failing tests**

```rust
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
```

- [ ] **Step 2: Run tests — expect FAIL**

```bash
cargo test -p als-engine remove_device_ -- --nocapture
```

- [ ] **Step 3: Implement `remove_device`**

```rust
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
```

- [ ] **Step 4: Run tests — expect PASS**

```bash
cargo test -p als-engine remove_device_ -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add engine/src/device_manager.rs
git commit -m "feat(engine): remove disconnected devices only"
```

---

### Task 3: Merge refresh (non-destructive)

**Files:**
- Modify: `engine/src/device_manager.rs`
- Update existing test `refresh_replaces_device_state` to match merge semantics

- [ ] **Step 1: Write failing merge tests; update old replace test**

Replace `refresh_replaces_device_state` with merge-focused tests:

```rust
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
```

Note: `from_adb_devices_with_log_root` is currently private (`fn`). Either:

1. Make it `pub(crate)` for tests, or  
2. Use `from_adb_devices` only (recorder writes under `logs/`) and avoid tempdir.

Prefer making the existing private helper callable from tests by changing to:

```rust
pub(crate) fn from_adb_devices_with_log_root(
```

if not already public enough within the module tests (same module can already call private fns — tests are inside `mod tests` in the same file, so **private `fn from_adb_devices_with_log_root` is already accessible**).

- [ ] **Step 2: Run tests — expect FAIL**

```bash
cargo test -p als-engine merge_scan_ -- --nocapture
```

- [ ] **Step 3: Implement `merge_scan_result` and route `refresh` through it**

Add method (sync merge of device list + contexts; logcat restart remains async in `refresh`):

```rust
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
        for device in self.devices.iter_mut() {
            if device.source == DeviceSource::Adb {
                device.connected = false;
            }
        }
        self.stop_logcat_children();
        self.adb_status = AdbStatus {
            available: path.as_ref().map(|_| true).unwrap_or(false),
            mode: if self.devices.iter().any(|d| d.source == DeviceSource::Adb) {
                AdbStatusMode::Bundled
            } else {
                AdbStatusMode::MockFallback
            },
            path: path.clone(),
            message: format!("ADB: {error}"),
        };
        // Prefer keeping ADB list when we already have ADB contexts.
        if !self.devices.iter().any(|d| d.source == DeviceSource::Adb) {
            self.switch_to_mock_fallback(format!("ADB: {error}, using mock device"));
        }
        return;
    }

    // Empty scan with existing ADB devices → soft-disconnect all ADB; do not force mock.
    if devices.is_empty() {
        for device in self.devices.iter_mut() {
            if device.source == DeviceSource::Adb {
                device.connected = false;
            }
        }
        self.stop_logcat_children();
        let path_string = path.unwrap_or_else(|| "libs/<platform>/adb".to_string());
        if self.devices.iter().any(|d| d.source == DeviceSource::Adb) {
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
        .map(|d| (d.serial.clone(), d))
        .collect();

    // Soft-disconnect ADB devices missing from scan
    let existing_ids: Vec<String> = self
        .devices
        .iter()
        .filter(|d| d.source == DeviceSource::Adb)
        .map(|d| d.device_id.clone())
        .collect();
    for id in &existing_ids {
        if !scanned.contains_key(id) {
            self.mark_disconnected(id);
        }
    }

    // Update or add scanned devices
    for (serial, adb_device) in &scanned {
        if let Some(device) = self.devices.iter_mut().find(|d| d.device_id == *serial) {
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

    let online = self.devices.iter().filter(|d| d.connected).count();
    self.adb_status = AdbStatus {
        available: true,
        mode: AdbStatusMode::Bundled,
        path: Some(path_string),
        message: format!("ADB: {online} device(s) connected"),
    };
}
```

Update async `refresh` to merge then start logcat only for connected ADB devices that lack a running child:

```rust
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
            self.merge_scan_result(Some(adb_path_string), Vec::new(), Some(error.to_string()));
        }
    }
}
```

Add `ensure_logcat_for_connected`:

```rust
async fn ensure_logcat_for_connected(&mut self, adb_path: &std::path::Path) {
    // Ensure log_receiver channel exists
    if self.log_receiver.is_none() {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.log_receiver = Some(receiver);
        // store sender for new children — need a field OR rebuild channel carefully.
    }
    // ...
}
```

**Channel caveat:** Today `start_logcat_processes` creates a fresh `(sender, receiver)` and replaces `log_receiver`. For merge refresh, simplest approach that stays correct:

1. `stop_logcat_children_async` for children that will be restarted only  
2. Or: always on merge refresh, **kill all logcat children**, create a **new** channel, re-spawn logcat for every `connected && source==Adb` device, assign new `log_receiver`.

Contexts/buffers stay. Implementation:

```rust
async fn ensure_logcat_for_connected(&mut self, adb_path: &std::path::Path) {
    self.stop_logcat_children_async().await;

    let online: Vec<String> = self
        .devices
        .iter()
        .filter(|d| d.source == DeviceSource::Adb && d.connected)
        .map(|d| d.device_id.clone())
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
                // Soft-fail one device: mark disconnected, continue others
                self.mark_disconnected(&device_id);
                self.adb_status.message =
                    format!("ADB: failed to start logcat for {device_id}: {error}");
            }
        }
    }
}
```

Deprecate destructive path: change `replace_with_scan_result` to call `merge_scan_result` (or delete call sites). Update any test that relied on full wipe.

Also update `adb_status` helper after soft disconnects in poll if desired:

```rust
pub fn refresh_adb_status_message(&mut self) {
    if self.is_mock_fallback() {
        return;
    }
    let online = self.devices.iter().filter(|d| d.connected).count();
    let disconnected = self.devices.iter().filter(|d| !d.connected).count();
    self.adb_status.message = if disconnected == 0 {
        format!("ADB: {online} device(s) connected")
    } else {
        format!("ADB: {online} online, {disconnected} disconnected")
    };
}
```

Call from `poll_logcat_exits` when dirty.

- [ ] **Step 4: Run all device_manager tests**

```bash
cargo test -p als-engine device_manager -- --nocapture
```

Expected: PASS (fix any tests that assumed wipe-on-refresh).

- [ ] **Step 5: Commit**

```bash
git add engine/src/device_manager.rs
git commit -m "feat(engine): merge device refresh without wiping buffers"
```

---

### Task 4: WebSocket — `connect_device`, `remove_device`, poll on tick

**Files:**
- Modify: `engine/src/websocket.rs`

- [ ] **Step 1: Add ClientMessage variant + deserialize test**

In `ClientMessage` enum:

```rust
RemoveDevice {
    device_id: String,
},
```

Test:

```rust
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
```

- [ ] **Step 2: Run test — expect PASS for deserialize once variant added; handlers still incomplete**

```bash
cargo test -p als-engine remove_device_message_deserializes -- --nocapture
```

- [ ] **Step 3: Wire handlers and tick poll**

In `handle_client_text`, replace the combined Connect/Disconnect stub:

```rust
Ok(ClientMessage::ConnectDevice { device_id }) => {
    if !manager.has_device(&device_id) {
        return send_error(sender, format!("unknown device: {device_id}")).await;
    }
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
```

In `handle_socket` ticker branch (ADB path), poll exits before draining logs:

```rust
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
```

Ensure `refresh_adb_status_message` exists (Task 3) or inline message update.

- [ ] **Step 4: Run websocket + engine tests**

```bash
cargo test -p als-engine -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add engine/src/websocket.rs engine/src/device_manager.rs
git commit -m "feat(engine): wire connect_device snapshot and remove_device"
```

---

### Task 5: Frontend protocol + store `device_list` rules

**Files:**
- Modify: `src/renderer/types/protocol.ts`
- Modify: `src/renderer/state/appStore.ts`
- Modify: `src/renderer/state/appStore.test.ts`

- [ ] **Step 1: Add protocol type**

In `ClientMessage` union:

```ts
| { type: 'remove_device'; deviceId: string }
```

- [ ] **Step 2: Write failing store tests**

In `appStore.test.ts`:

```ts
it('device_list connected flip keeps logs for same active device', () => {
  const { handleServerMessage } = useAppStore.getState();
  handleServerMessage({ type: 'device_list', devices: [deviceA, deviceB] });
  handleServerMessage({
    type: 'new_logs',
    deviceId: deviceA.deviceId,
    logs: [logEntry(1)],
  });

  handleServerMessage({
    type: 'device_list',
    devices: [{ ...deviceA, connected: false }, deviceB],
  });

  expect(useAppStore.getState().activeDeviceId).toBe(deviceA.deviceId);
  expect(useAppStore.getState().logs.map((l) => l.seq)).toEqual([1]);
  expect(
    useAppStore.getState().devices.find((d) => d.deviceId === deviceA.deviceId)?.connected,
  ).toBe(false);
});

it('device_list removing active device switches and clears logs', () => {
  const { handleServerMessage } = useAppStore.getState();
  handleServerMessage({ type: 'device_list', devices: [deviceA, deviceB] });
  handleServerMessage({
    type: 'new_logs',
    deviceId: deviceA.deviceId,
    logs: [logEntry(1)],
  });

  handleServerMessage({ type: 'device_list', devices: [deviceB] });

  expect(useAppStore.getState().activeDeviceId).toBe(deviceB.deviceId);
  expect(useAppStore.getState().logs).toEqual([]);
});
```

- [ ] **Step 3: Run tests — expect FAIL on connected-flip case**

```bash
npm test -- src/renderer/state/appStore.test.ts
```

Current `device_list` handler already keeps active if still in list and only empties when active id changes — verify the connected-flip test; if it already passes, good. If `nextActiveDeviceId` or empty state incorrectly clears, fix as below.

- [ ] **Step 4: Confirm / fix `device_list` handler**

In `appStore.ts` `case 'device_list':` ensure:

```ts
case 'device_list':
  set((state) => {
    const activeDeviceId = nextActiveDeviceId(state.activeDeviceId, message.devices);
    const activeChanged = activeDeviceId !== state.activeDeviceId;
    return {
      devices: message.devices,
      activeDeviceId,
      connected: true,
      ...(activeChanged ? emptyActiveDeviceState() : {}),
    };
  });
  break;
```

Do **not** clear when only `connected` flags change.

- [ ] **Step 5: Run tests — PASS**

```bash
npm test -- src/renderer/state/appStore.test.ts
```

- [ ] **Step 6: Commit**

```bash
git add src/renderer/types/protocol.ts src/renderer/state/appStore.ts src/renderer/state/appStore.test.ts
git commit -m "feat(ui): keep logs on soft disconnect device_list updates"
```

---

### Task 6: Frontend switch → `connect_device`, remove UI, i18n

**Files:**
- Modify: `src/renderer/App.tsx`
- Modify: `src/renderer/components/DeviceSelect.tsx`
- Modify: `src/renderer/settings/i18n.ts`
- Modify: `src/renderer/styles.css` (minimal if needed)

- [ ] **Step 1: i18n keys**

Add to `MessageKey`:

```ts
| 'deviceDisconnected'
| 'removeDevice'
```

English:

```ts
deviceDisconnected: 'Disconnected',
removeDevice: 'Remove device',
```

Chinese:

```ts
deviceDisconnected: '已断开',
removeDevice: '移除设备',
```

(Existing `disconnected` is for WS connection status — keep separate.)

- [ ] **Step 2: DeviceSelect labels**

```tsx
{devices.map((device) => (
  <option key={device.deviceId} value={device.deviceId}>
    {device.deviceName} · {device.deviceId}
    {device.connected
      ? ` (${device.source.toUpperCase()})`
      : ` (${t(locale, 'deviceDisconnected')})`}
  </option>
))}
```

- [ ] **Step 3: App — switch sends `connect_device`; remove button**

```tsx
const handleDeviceChange = useCallback(
  (deviceId: string) => {
    setActiveDeviceId(deviceId);
    clientRef.current?.send({ type: 'connect_device', deviceId });
  },
  [setActiveDeviceId],
);

const handleRemoveDevice = useCallback(() => {
  if (!activeDeviceId) return;
  const device = devices.find((d) => d.deviceId === activeDeviceId);
  if (!device || device.connected) return;
  clientRef.current?.send({ type: 'remove_device', deviceId: activeDeviceId });
}, [activeDeviceId, devices]);
```

When `device_list` changes active id (store), App must also request snapshot for the new active. Extend the existing filter effect or add:

```tsx
useEffect(() => {
  if (!activeDeviceId || !connected) return;
  clientRef.current?.send({ type: 'connect_device', deviceId: activeDeviceId });
  sendFilter(packageFilter, tagFilter, selectedLevels);
}, [activeDeviceId, connected]); // eslint-disable-line ...
```

**Important:** Avoid double-send on first mount if possible; acceptable per spec.

Remove button in toolbar (near device select):

```tsx
const activeDevice = devices.find((d) => d.deviceId === activeDeviceId);
const canRemove = Boolean(activeDevice && !activeDevice.connected);

// ...
<button
  className="toolbar-btn"
  type="button"
  onClick={handleRemoveDevice}
  disabled={!canRemove}
  title={t(locale, 'removeDevice')}
>
  {t(locale, 'removeDevice')}
</button>
```

- [ ] **Step 4: Build / unit tests**

```bash
npm test
npm run build
```

Expected: PASS (or fix TS errors from new ClientMessage / i18n keys).

- [ ] **Step 5: Commit**

```bash
git add src/renderer/App.tsx src/renderer/components/DeviceSelect.tsx src/renderer/settings/i18n.ts src/renderer/styles.css
git commit -m "feat(ui): device switch snapshot and remove disconnected"
```

---

### Task 7: Full verification

- [ ] **Step 1: Engine tests**

```bash
cargo test -p als-engine
```

Expected: all PASS.

- [ ] **Step 2: Frontend tests + build**

```bash
npm test
npm run build
```

Expected: PASS.

- [ ] **Step 3: e2e if environment allows**

```bash
npm run test:e2e
```

Expected: PASS or document pre-existing failures unrelated to this feature.

- [ ] **Step 4: Manual checklist (if device available)**

1. Two devices (or mock + real): switch → list fills without waiting for new lines  
2. Kill logcat / unplug → active device shows 已断开, logs remain  
3. Remove → device gone; other device still works  
4. Refresh with device back → reappears, buffers for never-removed devices intact  

- [ ] **Step 5: Final commit if any fixups**

```bash
git add -A  # only files related to this feature
git status  # review
git commit -m "test: cover device disconnect and switch isolation"
```

---

## Spec coverage checklist

| Spec requirement | Task |
|---|---|
| Switch → `log_snapshot` via `connect_device` | 4, 6 |
| Soft disconnect on logcat exit | 1, 4 |
| Soft disconnect on Refresh missing device | 3 |
| Keep history when viewing disconnected | 1, 5 |
| `remove_device` only if `!connected` | 2, 4, 6 |
| Refresh merge without wiping buffers | 3 |
| Frontend single `logs` array | 5, 6 (no Map) |
| DeviceSelect disconnected label | 6 |
| No auto-reconnect | 1, 3 (no reconnect without refresh) |
| `disconnect_device` stays stub | 4 |
| Tests engine + store | 1–5, 7 |

## Placeholder / consistency notes

- Method names used throughout: `mark_disconnected`, `poll_logcat_exits`, `remove_device`, `merge_scan_result`, `ensure_logcat_for_connected`, `refresh_adb_status_message`, `connect_device` / `remove_device` WS types.
- `from_adb_devices_with_log_root` remains callable from unit tests in the same module.
- Double snapshot on switch (`connect_device` + `set_filter`) is accepted by design.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-15-device-disconnect-and-log-isolation.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — this session runs tasks with executing-plans checkpoints  

Which approach?
