# Log Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users export the active device’s engine ring buffer as threadtime plain text—either **all** lines or **filter-visible** lines—via a system Save As dialog.

**Architecture:** Engine walks the per-device buffer, writes a temp file under `logs/exports/`, replies with `export_ready { path, lineCount, mode }`. Renderer invokes Electron IPC `export:save` → main process `showSaveDialog` + `copyFile` + unlink temp. UI has two buttons: Export all / Export filtered.

**Tech Stack:** Rust (`als-engine`, ring buffer, axum WS), Electron main IPC + `dialog`, React toolbar, TypeScript protocol, cargo test + vitest.

**Spec:** `docs/superpowers/specs/2026-07-16-log-export-design.md`

**Worktree:** `/home/noah/Codes/PC/AndroidLogcatStudio/.worktrees/device-disconnect`  
**Branch:** `feature/device-disconnect-log-isolation`

---

## File map

| File | Responsibility |
|------|----------------|
| `engine/src/log_entry.rs` | `LogLevel::as_threadtime_char()` helper |
| `engine/src/device.rs` | Format line + `export_logs(mode, path)` on `DeviceContext` |
| `engine/src/device_manager.rs` | `export_logs(device_id, mode)` → temp path under `logs/exports` |
| `engine/src/websocket.rs` | `ExportLogs` client msg, `ExportReady` server msg, handler |
| `src/renderer/types/protocol.ts` | TS protocol + `window.als.exportSave` |
| `src/main/preload.cjs` | Expose `exportSave` |
| `src/main/main.ts` | `export:save` IPC (allowlist, Save As, copy, unlink) |
| `src/renderer/export/fileName.ts` | Default export filename helper (pure) |
| `src/renderer/App.tsx` | Buttons + export flow |
| `src/renderer/styles.css` | Minimal button layout if needed |

---

### Task 1: Threadtime line format + DeviceContext export

**Files:**
- Modify: `engine/src/log_entry.rs`
- Modify: `engine/src/device.rs`
- Test: `engine/src/device.rs` `mod tests`

- [ ] **Step 1: Write failing tests for format and export modes**

In `engine/src/device.rs` tests module, add (adapt imports to match existing test helpers):

```rust
#[test]
fn format_threadtime_line_matches_parser_roundtrip_shape() {
    let line = format_threadtime_line(&LogEntry {
        seq: 1,
        timestamp: 0,
        date: "07-16".into(),
        time: "12:34:56.789".into(),
        pid: 1234,
        tid: 5678,
        level: LogLevel::Info,
        tag: "ActivityManager".into(),
        message: "hello".into(),
        package_name: None,
        foreground: None,
        background: None,
        hidden: false,
        bookmarked: false,
    });
    assert_eq!(line, "07-16 12:34:56.789  1234  5678 I ActivityManager: hello");
}

#[test]
fn export_all_includes_hidden_filtered_skips_them() {
    let dir = tempdir().expect("tempdir");
    let mut device = new_test_device(100);
    // two lines then filter to hide non-matching
    assert!(device.ingest_line("07-16 12:00:00.000  1  1 I Keep: stay").is_some());
    assert!(device.ingest_line("07-16 12:00:01.000  1  1 I Drop: gone").is_some());
    device.set_filter(FilterQuery::parse("tag:Keep"));

    let all_path = dir.path().join("all.log");
    let filtered_path = dir.path().join("filtered.log");
    let all = device
        .export_logs(ExportMode::All, &all_path)
        .expect("export all");
    let filtered = device
        .export_logs(ExportMode::Filtered, &filtered_path)
        .expect("export filtered");

    assert_eq!(all.line_count, 2);
    assert_eq!(filtered.line_count, 1);
    let all_text = std::fs::read_to_string(&all_path).unwrap();
    let filtered_text = std::fs::read_to_string(&filtered_path).unwrap();
    assert!(all_text.contains("Keep: stay"));
    assert!(all_text.contains("Drop: gone"));
    assert!(filtered_text.contains("Keep: stay"));
    assert!(!filtered_text.contains("Drop: gone"));
}

#[test]
fn export_empty_buffer_writes_zero_lines() {
    let dir = tempdir().expect("tempdir");
    let device = new_test_device(10);
    let path = dir.path().join("empty.log");
    let result = device
        .export_logs(ExportMode::All, &path)
        .expect("empty ok");
    assert_eq!(result.line_count, 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
}
```

Ensure `new_test_device` and `FilterQuery` imports already exist in the module; add:

```rust
use crate::filter::FilterQuery;
use crate::log_entry::LogLevel;
use super::{ExportMode, format_threadtime_line};
// ExportResult if asserted
```

- [ ] **Step 2: Run tests — expect FAIL**

```bash
cd /home/noah/Codes/PC/AndroidLogcatStudio/.worktrees/device-disconnect
cargo test -p als-engine format_threadtime_line_ -- --nocapture
cargo test -p als-engine export_all_includes_hidden -- --nocapture
```

Expected: compile fail (symbols missing) or FAIL.

- [ ] **Step 3: Implement level char, format, ExportMode, DeviceContext::export_logs**

In `engine/src/log_entry.rs` on `LogLevel`:

```rust
impl LogLevel {
    pub fn as_threadtime_char(self) -> char {
        match self {
            LogLevel::Verbose => 'V',
            LogLevel::Debug => 'D',
            LogLevel::Info => 'I',
            LogLevel::Warn => 'W',
            LogLevel::Error => 'E',
            LogLevel::Assert => 'A',
            LogLevel::Unknown => '?',
        }
    }
}
```

In `engine/src/device.rs`:

```rust
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    All,
    Filtered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportResult {
    pub line_count: usize,
}

pub fn format_threadtime_line(entry: &LogEntry) -> String {
    format!(
        "{} {}  {:>5}  {:>5} {} {}: {}",
        entry.date,
        entry.time,
        entry.pid,
        entry.tid,
        entry.level.as_threadtime_char(),
        entry.tag,
        entry.message
    )
}

impl DeviceContext {
    pub fn export_logs(&self, mode: ExportMode, path: &Path) -> anyhow::Result<ExportResult> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        let mut line_count = 0usize;
        for entry in self.buffer.latest(usize::MAX) {
            let include = match mode {
                ExportMode::All => true,
                ExportMode::Filtered => !entry.hidden,
            };
            if !include {
                continue;
            }
            writeln!(writer, "{}", format_threadtime_line(&entry))?;
            line_count += 1;
        }
        writer.flush()?;
        Ok(ExportResult { line_count })
    }
}
```

- [ ] **Step 4: Run tests — expect PASS**

```bash
cargo test -p als-engine format_threadtime_line_ export_all_includes_hidden export_empty_buffer -- --nocapture
```

Expected: PASS. If filter query syntax differs, adjust test filter to match `FilterQuery::parse` (check `engine/src/filter.rs` for tag syntax).

- [ ] **Step 5: Commit**

```bash
git add engine/src/log_entry.rs engine/src/device.rs
git commit -m "feat(engine): export device buffer as threadtime text"
```

---

### Task 2: DeviceManager::export_logs → temp file path

**Files:**
- Modify: `engine/src/device_manager.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn manager_export_logs_unknown_device_errors() {
    let manager = DeviceManager::mock_fallback("test");
    let err = manager
        .export_logs("missing", ExportMode::All)
        .expect_err("unknown");
    assert!(err.to_string().contains("unknown device"));
}

#[test]
fn manager_export_logs_writes_under_exports_dir() {
    let log_root = tempdir().expect("tempdir");
    let mut manager = DeviceManager::mock_fallback_with_log_root(
        "test",
        log_root.path().to_path_buf(),
    );
    // If mock_fallback_with_log_root does not exist, use from_adb_devices_with_log_root
    // or mock_fallback and accept cwd logs/exports — prefer temp log_root if available.
    assert!(manager
        .ingest_mock_line("07-16 12:00:00.000  1  1 I Tag: line")
        .is_some());

    let result = manager
        .export_logs(MOCK_DEVICE_ID, ExportMode::All)
        .expect("export");
    assert_eq!(result.line_count, 1);
    assert!(result.path.exists());
    assert!(
        result
            .path
            .to_string_lossy()
            .contains("exports"),
        "path should be under exports: {:?}",
        result.path
    );
    let text = std::fs::read_to_string(&result.path).unwrap();
    assert!(text.contains("Tag: line"));
}
```

Check whether `mock_fallback_with_log_root` exists (used in device_manager tests). If only `from_adb_devices_with_log_root` exists, build a one-device ADB manager with temp root and ingest via context.

Also export public types:

```rust
use crate::device::{ExportMode, ExportResult as DeviceExportResult};
// or re-export ExportMode from device
```

`DeviceManager` return type:

```rust
pub struct ManagedExportResult {
    pub path: PathBuf,
    pub line_count: usize,
    pub mode: ExportMode,
}
```

- [ ] **Step 2: Run — expect FAIL**

```bash
cargo test -p als-engine manager_export_logs_ -- --nocapture
```

- [ ] **Step 3: Implement DeviceManager::export_logs**

```rust
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
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
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
```

**Note:** Prefer using the manager’s log root if the struct already has a field; if Recorder roots are per-device under `logs/`, keeping `logs/exports` under cwd is fine and matches the spec. Document in a one-line comment.

Re-export `ExportMode` from `device` for websocket:

```rust
pub use crate::device::ExportMode;
```

(or keep websocket importing from `device`).

- [ ] **Step 4: Run tests — PASS**

```bash
cargo test -p als-engine manager_export_logs_ -- --nocapture
cargo test -p als-engine -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add engine/src/device_manager.rs engine/src/device.rs
git commit -m "feat(engine): DeviceManager export_logs temp files"
```

---

### Task 3: WebSocket protocol + handler

**Files:**
- Modify: `engine/src/websocket.rs`

- [ ] **Step 1: Add messages + deserialize test**

In `ClientMessage`:

```rust
ExportLogs {
    device_id: String,
    mode: String,
},
```

In `ServerMessage`:

```rust
ExportReady {
    device_id: String,
    mode: String,
    path: String,
    line_count: usize,
},
```

Test:

```rust
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
```

- [ ] **Step 2: Run deserialize test — PASS once variants exist**

```bash
cargo test -p als-engine export_logs_message_deserializes export_ready_message_serializes -- --nocapture
```

- [ ] **Step 3: Wire handler**

In `handle_client_text`:

```rust
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
```

Import `ExportMode` from `device` or `device_manager`.

**Concurrency (minimal):** optional `std::sync::atomic::AtomicBool` on a static or skip this iteration if single-threaded WS loop already serializes messages per connection. Spec prefers reject concurrent — if one connection processes messages sequentially, natural serialization is enough. **Do not** add global mutex unless multi-connection concurrent exports are possible on shared manager. Current design: **one DeviceManager per socket** → sequential handling is enough; document in comment, no extra flag required.

- [ ] **Step 4: Full engine tests**

```bash
cargo test -p als-engine -- --nocapture
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add engine/src/websocket.rs
git commit -m "feat(engine): wire export_logs websocket handler"
```

---

### Task 4: TypeScript protocol + default filename helper

**Files:**
- Modify: `src/renderer/types/protocol.ts`
- Create: `src/renderer/export/fileName.ts`
- Create: `src/renderer/export/fileName.test.ts` (or add to existing vitest file)

- [ ] **Step 1: Extend protocol**

```ts
export type ExportMode = 'all' | 'filtered';

// ClientMessage add:
| { type: 'export_logs'; deviceId: string; mode: ExportMode }

// ServerMessage add:
| {
    type: 'export_ready';
    deviceId: string;
    mode: ExportMode;
    path: string;
    lineCount: number;
  }

// Window.als:
exportSave: (
  tempPath: string,
  defaultName: string,
) => Promise<{ canceled: boolean; path?: string; error?: string }>;
```

- [ ] **Step 2: Filename helper + test**

`src/renderer/export/fileName.ts`:

```ts
export function buildExportFileName(
  deviceLabel: string,
  mode: 'all' | 'filtered',
  now: Date = new Date(),
): string {
  const safe = deviceLabel.replace(/[^a-zA-Z0-9._-]+/g, '_').replace(/^_|_$/g, '') || 'device';
  const pad = (n: number) => String(n).padStart(2, '0');
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `${safe}-${mode}-${stamp}.log`;
}
```

Test:

```ts
import { describe, expect, it } from 'vitest';
import { buildExportFileName } from './fileName';

describe('buildExportFileName', () => {
  it('sanitizes device label and includes mode', () => {
    const name = buildExportFileName('Mock Device', 'filtered', new Date('2026-07-16T12:34:56'));
    expect(name).toBe('Mock_Device-filtered-20260716-123456.log');
  });
});
```

(Adjust expected if timezone makes local hours differ — pass fixed `Date` and use **local** getters; for CI stability, either mock timezone or only assert prefix/suffix with regex:

```ts
expect(name).toMatch(/^Mock_Device-filtered-\d{8}-\d{6}\.log$/);
```

Prefer regex for TZ safety.)

- [ ] **Step 3: Run vitest**

```bash
npm test
```

Expected: PASS (including new test).

- [ ] **Step 4: Commit**

```bash
git add src/renderer/types/protocol.ts src/renderer/export/fileName.ts src/renderer/export/fileName.test.ts
git commit -m "feat(ui): export protocol types and default filename"
```

---

### Task 5: Electron preload + main export:save IPC

**Files:**
- Modify: `src/main/preload.cjs`
- Modify: `src/main/main.ts`

- [ ] **Step 1: Preload**

```js
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('als', {
  version: '0.1.0',
  getEngineUrl: () => ipcRenderer.invoke('engine:get-url'),
  exportSave: (tempPath, defaultName) =>
    ipcRenderer.invoke('export:save', { tempPath, defaultName }),
});
```

- [ ] **Step 2: Main process handler**

At top of `main.ts` add:

```ts
import { app, BrowserWindow, ipcMain, dialog } from 'electron';
import fs from 'node:fs/promises';
import path from 'node:path';
```

After existing `ipcMain.handle('engine:get-url', ...)`:

```ts
function isAllowedExportTempPath(tempPath: string): boolean {
  const resolved = path.resolve(tempPath);
  const exportsDir = path.resolve(process.cwd(), 'logs', 'exports');
  return resolved === exportsDir || resolved.startsWith(exportsDir + path.sep);
}

ipcMain.handle(
  'export:save',
  async (
    _event,
    payload: { tempPath?: string; defaultName?: string },
  ): Promise<{ canceled: boolean; path?: string; error?: string }> => {
    const tempPath = payload?.tempPath;
    const defaultName = payload?.defaultName || 'export.log';
    if (!tempPath || typeof tempPath !== 'string') {
      return { canceled: false, error: 'missing tempPath' };
    }
    if (!isAllowedExportTempPath(tempPath)) {
      return { canceled: false, error: 'temp path not allowed' };
    }

    try {
      await fs.access(tempPath);
    } catch {
      return { canceled: false, error: 'temp file missing' };
    }

    const result = await dialog.showSaveDialog({
      defaultPath: defaultName,
      filters: [
        { name: 'Log files', extensions: ['log', 'txt'] },
        { name: 'All files', extensions: ['*'] },
      ],
    });

    if (result.canceled || !result.filePath) {
      await fs.unlink(tempPath).catch(() => undefined);
      return { canceled: true };
    }

    try {
      await fs.copyFile(tempPath, result.filePath);
      await fs.unlink(tempPath).catch(() => undefined);
      return { canceled: false, path: result.filePath };
    } catch (error) {
      await fs.unlink(tempPath).catch(() => undefined);
      const message = error instanceof Error ? error.message : String(error);
      return { canceled: false, error: message };
    }
  },
);
```

- [ ] **Step 3: Build main**

```bash
npm run build
```

Expected: PASS (or TS only). Fix type errors on `window.als` if preload types lag — protocol.ts Window interface already updated in Task 4.

- [ ] **Step 4: Commit**

```bash
git add src/main/preload.cjs src/main/main.ts
git commit -m "feat(electron): Save As IPC for log export"
```

---

### Task 6: App UI — Export all / Export filtered

**Files:**
- Modify: `src/renderer/App.tsx`
- Modify: `src/renderer/styles.css` (only if needed for button row)

- [ ] **Step 1: Export handlers**

Add imports:

```ts
import { buildExportFileName } from './export/fileName';
import type { ExportMode, ServerMessage } from './types/protocol';
```

State:

```ts
const [exportBusy, setExportBusy] = useState(false);
const [exportHint, setExportHint] = useState<string | null>(null);
const statusWarning =
  [recorderWarning, refreshWarning, exportHint].filter(Boolean).join(' · ') || null;
```

Pending export promise map (module-level or ref):

```ts
// Inside App component:
const pendingExportRef = useRef<{
  deviceId: string;
  mode: ExportMode;
  resolve: (msg: Extract<ServerMessage, { type: 'export_ready' }>) => void;
  reject: (err: Error) => void;
} | null>(null);

// Wrap handleServerMessage or branch in a local handler:
const onServerMessage = useCallback(
  (message: ServerMessage) => {
    if (message.type === 'export_ready') {
      const pending = pendingExportRef.current;
      if (
        pending &&
        pending.deviceId === message.deviceId &&
        pending.mode === message.mode
      ) {
        pendingExportRef.current = null;
        pending.resolve(message);
      }
    }
    if (message.type === 'error' && pendingExportRef.current) {
      // Only reject if we are waiting — might steal unrelated errors.
      // Prefer: timeout-based wait; or include export correlation later.
      // Spec: use error message if export in flight.
      const pending = pendingExportRef.current;
      pendingExportRef.current = null;
      pending.reject(new Error(message.message));
    }
    handleServerMessage(message);
  },
  [handleServerMessage],
);
```

**Important:** Wire `EngineClient` with `onServerMessage` instead of raw `handleServerMessage` in the connect effect.

Safer wait pattern without stealing global errors:

```ts
function waitForExportReady(
  deviceId: string,
  mode: ExportMode,
  timeoutMs = 60_000,
): Promise<Extract<ServerMessage, { type: 'export_ready' }>> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      if (pendingExportRef.current) {
        pendingExportRef.current = null;
        reject(new Error('export timed out'));
      }
    }, timeoutMs);
    pendingExportRef.current = {
      deviceId,
      mode,
      resolve: (msg) => {
        clearTimeout(timer);
        resolve(msg);
      },
      reject: (err) => {
        clearTimeout(timer);
        reject(err);
      },
    };
  });
}
```

Only resolve on `export_ready` match; leave generic `error` messages to status via store, and reject pending only if message looks export-related (`message.includes('export') || message.includes('unknown device')`) — **or** always reject pending on any error while busy (simpler, acceptable this iteration):

```ts
if (message.type === 'error' && pendingExportRef.current && exportBusy) {
  ...
}
```

But `exportBusy` in closure is stale — use `pendingExportRef.current` alone: any error while pending rejects. Document that concurrent non-export errors during export will cancel wait.

```ts
const runExport = useCallback(
  async (mode: ExportMode) => {
    if (!activeDeviceId || exportBusy) return;
    setExportBusy(true);
    setExportHint(null);
    try {
      const wait = waitForExportReady(activeDeviceId, mode);
      const sent = clientRef.current?.send({
        type: 'export_logs',
        deviceId: activeDeviceId,
        mode,
      });
      if (!sent) {
        pendingExportRef.current = null;
        throw new Error('Unable to export while disconnected');
      }
      const ready = await wait;
      const device = devices.find((d) => d.deviceId === activeDeviceId);
      const defaultName = buildExportFileName(device?.deviceName ?? activeDeviceId, mode);
      const saved = await window.als.exportSave(ready.path, defaultName);
      if (saved.canceled) {
        setExportHint('Export canceled');
      } else if (saved.error) {
        setExportHint(`Export failed: ${saved.error}`);
      } else {
        setExportHint(`Exported ${ready.lineCount} lines`);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setExportHint(message);
    } finally {
      setExportBusy(false);
    }
  },
  [activeDeviceId, devices, exportBusy],
);
```

- [ ] **Step 2: Buttons in toolbar** (next to Refresh / Remove)

```tsx
const canExport = Boolean(activeDeviceId && connected && !exportBusy);

<button
  className="refresh-devices"
  type="button"
  disabled={!canExport}
  onClick={() => void runExport('all')}
>
  Export all
</button>
<button
  className="refresh-devices"
  type="button"
  disabled={!canExport}
  onClick={() => void runExport('filtered')}
>
  Export filtered
</button>
```

- [ ] **Step 3: Build + unit tests**

```bash
npm test
npm run build
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/renderer/App.tsx src/renderer/styles.css
git commit -m "feat(ui): export all and filtered log buttons"
```

---

### Task 7: Verification

- [ ] **Step 1: Engine tests**

```bash
cargo test -p als-engine
```

Expected: all PASS.

- [ ] **Step 2: Frontend + build**

```bash
npm test
npm run build
```

Expected: PASS.

- [ ] **Step 3: Optional e2e**

```bash
npm run test:e2e
```

Expected: existing e2e still PASS (mock WS may ignore export). No requirement to automate Save dialog.

- [ ] **Step 4: Manual checklist**

1. Launch app (mock): **Export all** → Save As → open file → mock lines present.  
2. Apply query filter → **Export filtered** → fewer lines.  
3. Cancel Save As → no leftover under chosen path; `logs/exports` temp cleaned.  
4. Soft-disconnect device (if available) still exports.

- [ ] **Step 5: Final commit if fixups**

```bash
git status
# commit only export-related fixes
```

---

## Spec coverage checklist

| Spec requirement | Task |
|------------------|------|
| Export all (incl. hidden) | 1, 2, 3, 6 |
| Export filtered (`!hidden`, no UI cap) | 1, 2, 3, 6 |
| Threadtime plain text | 1 |
| Temp under `logs/exports` + path in `export_ready` | 2, 3 |
| Save As dialog + copy + unlink | 5, 6 |
| Soft-disconnected exportable | 2 (`has_device` / context kept) |
| Empty buffer lineCount 0 | 1 |
| Path allowlist | 5 |
| Filename sanitize | 2, 4 |
| No JSON / multi-device / clipboard | — (not planned) |
| Tests | 1, 2, 3, 4, 7 |

## Placeholder / consistency notes

- Types: `ExportMode` = `All`/`Filtered` in Rust; `"all"`/`"filtered"` on wire and TS.
- Server field `line_count` → JSON `lineCount` via existing `rename_all_fields = "camelCase"`.
- `format_threadtime_line` is free function in `device.rs` (or `log_entry.rs`); keep tests importing one place.
- One manager per WS connection → no export mutex required this iteration.
- `export_ready` waiter in App must not permanently swallow unrelated `error` messages after timeout clear.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-16-log-export.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — this session runs tasks with executing-plans checkpoints  

Which approach?
