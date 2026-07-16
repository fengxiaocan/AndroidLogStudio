# Log Export Design

**Date:** 2026-07-16  
**Status:** Design approved, pending implementation plan  
**Scope:** On-demand export of engine-buffered logs for the active device as logcat threadtime text via system Save dialog  
**Branch context:** Builds on `feature/device-disconnect-log-isolation` (engine as source of truth per device)

---

## 1. Goal

Add two export modes for the **current active device**:

| Mode | Content |
|------|---------|
| **All** | Every entry currently in that device’s engine ring buffer (includes filter-hidden rows) |
| **Filtered** | Every entry in the ring buffer with `hidden == false` under the **current engine filter** (not capped by UI `visibleLimit`) |

**Format:** plain-text logcat **threadtime**-style lines (readable, greppable, close to `adb logcat` / on-disk Recorder).  
**Destination:** system **Save As** dialog (user chooses path and filename).

**Out of scope (YAGNI)**

- JSON / CSV / NDJSON
- Multi-device batch export
- Clipboard export
- Progress bar for multi-million-line dumps
- Changing ring buffer capacity
- Auto-open exported file after save
- Exporting only UI-capped Zustand `logs` as a separate path (UI list export **is** the filtered engine path)

---

## 2. Current state (relevant)

| Layer | Behavior |
|-------|----------|
| Engine | Per-device `DeviceContext` + `RingBuffer` (~1_000_000 capacity); filter marks `entry.hidden` |
| Snapshot | `latest_visible_snapshot(limit)` returns non-hidden, last `limit` rows only |
| UI | Single `logs: LogEntry[]` for active device, sliced by `visibleLimit` (500–5000) |
| Disk today | Continuous **Recorder** writes raw ingest lines under `logs/…` — not user-triggered export |
| Electron | No `dialog.showSaveDialog` / file-save IPC yet; preload exposes `getEngineUrl` only |

Gaps: no full-buffer dump API; no filtered dump without limit; no Save As path.

---

## 3. Architecture (recommended)

```
UI: Export all | Export filtered
  → WS: export_logs { deviceId, mode: "all" | "filtered" }
  → Engine: walk ring buffer → write temp threadtime file under logs/exports/
  → WS: export_ready { deviceId, mode, path, lineCount }
  → Renderer: IPC export:save(tempPath, defaultName)
  → Main: showSaveDialog → copyFile(temp → user path) → unlink temp
  → UI: status / toast with lineCount or error
```

**Why not stream chunks over WS or dump only frontend `logs`:**  
Million-row payloads over JSON risk memory and UI jank; UI array is truncated. Temp file + copy keeps engine as authority and bounds IPC to a path string.

---

## 4. Protocol

### 4.1 Client

```ts
| { type: 'export_logs'; deviceId: string; mode: 'all' | 'filtered' }
```

### 4.2 Server

```ts
| {
    type: 'export_ready';
    deviceId: string;
    mode: 'all' | 'filtered';
    path: string;      // absolute temp file path on engine host (same machine)
    lineCount: number;
  }
```

Errors use existing `{ type: 'error', message: string }` (unknown device, IO failure, etc.).

### 4.3 Rules

- `deviceId` must exist (`has_device`); soft-disconnected devices **are** exportable (history still in buffer).
- Mode `filtered` uses current engine filter flags (`!hidden`); does **not** apply UI search-query highlighting.
- Empty buffer → still succeed with `lineCount: 0` and an empty (or header-less) temp file; UI may show “0 lines exported”.
- Concurrent exports: **serialize per process** (simple mutex / “export in progress” flag). Second request → error `export already in progress` or queue one; prefer **reject** for simplicity this iteration.

---

## 5. Engine behavior

### 5.1 API shape

```rust
pub enum ExportMode { All, Filtered }

pub struct ExportResult {
    pub path: PathBuf,
    pub line_count: usize,
    pub mode: ExportMode,
}

impl DeviceManager {
    pub fn export_logs(&self, device_id: &str, mode: ExportMode) -> anyhow::Result<ExportResult>;
}
```

Implementation details (plan may refine):

1. Resolve `DeviceContext` or bail `unknown device`.
2. Create directory `logs/exports` (relative to process cwd / existing log root if one is threaded later — default `logs/exports` under cwd for parity with Recorder).
3. Temp filename: `{device_id}-{mode}-{unix_ms}.log` (sanitize device_id for path safety).
4. Iterate buffer in chronological order (`latest(capacity)` or direct iter oldest→newest).
5. For each entry matching mode, write one line + `\n`.
6. Flush and return absolute path + count.

### 5.2 Line format (threadtime-style)

Prefer reconstructing from `LogEntry` fields already parsed:

```text
{date} {time}  {pid:>5}  {tid:>5} {level_char} {tag}: {message}
```

- `level_char`: first letter of level in logcat convention (`V/D/I/W/E/A/?` for unknown).
- `date` / `time` as stored on the entry (already threadtime components from parser).
- Do **not** re-fetch raw line from Recorder; export is buffer-based.

Hidden rows:

- **All:** include every entry regardless of `hidden`.
- **Filtered:** skip `entry.hidden`.

### 5.3 WebSocket handler

On `export_logs`:

1. Map mode string → `ExportMode`.
2. Call `manager.export_logs`.
3. Ok → `export_ready`; Err → `error`.

Does not start/stop logcat. Does not mutate filter or buffer.

---

## 6. Electron main / preload

### 6.1 Preload (CJS) additions

```js
exportSave: (tempPath, defaultName) =>
  ipcRenderer.invoke('export:save', { tempPath, defaultName }),
```

### 6.2 Main process

`ipcMain.handle('export:save', async (_e, { tempPath, defaultName }) => { ... })`:

1. Validate `tempPath` is under allowed export temp dir (must contain `logs/exports` or a known prefix) to avoid arbitrary file read.
2. `dialog.showSaveDialog` with `defaultPath: defaultName`, filters: `[{ name: 'Log', extensions: ['log', 'txt'] }]`.
3. If canceled → delete temp file, return `{ canceled: true }`.
4. Else `fs.promises.copyFile(tempPath, userPath)`, unlink temp, return `{ canceled: false, path: userPath }`.
5. On copy failure → try unlink temp, return / throw error for renderer.

Default name suggestion from renderer:  
`{deviceName or deviceId}-{all|filtered}-{YYYYMMDD-HHmmss}.log` (filesystem-safe).

### 6.3 Renderer flow

```
send export_logs
await export_ready for matching deviceId+mode (or error)
call window.als.exportSave(path, defaultName)
show outcome (status bar warning/info or brief console — match existing StatusBar patterns)
```

Race: if user switches device mid-export, still save the file for the **requested** deviceId from the response (do not cancel solely because active tab changed).

---

## 7. UI

- Two toolbar buttons next to Refresh / Remove, or one split control:
  - **Export all**
  - **Export filtered**
- Disabled when: no `activeDeviceId`, or WebSocket disconnected, or export in flight.
- English labels this iteration (feature branch has no i18n); if later merged with settings/i18n, add keys then.
- Optional: set `refreshWarning` / status warning string to `Exported N lines` or error text (reuse existing warning channel rather than new toast system).

---

## 8. Testing

| Layer | Cases |
|-------|--------|
| Engine unit | all includes hidden; filtered excludes hidden; empty buffer line_count 0; unknown device errs; file non-empty when lines present; first/last line format smoke |
| WS (optional) | deserialize `export_logs`; handler returns ready shape |
| Renderer (light) | default filename helper; ignore if no pure function |

No requirement for Playwright e2e of native Save dialog this iteration (hard to automate); manual checklist:

1. Mock device, Export all → file opens with mock lines  
2. Apply filter, Export filtered → fewer lines, no hidden tags  
3. Cancel Save As → no leftover user file; temp cleaned  
4. Soft-disconnected device still exportable  

---

## 9. Security / robustness

- Path allowlist for temp files before copy.
- Sanitize device_id in temp filenames (`/` → `_`).
- Do not send full log bodies over WS.
- Large exports may block the engine thread briefly while writing; acceptable this iteration (same process as logcat drain). Optional later: spawn blocking task on `spawn_blocking`.

---

## 10. Spec coverage checklist

| Requirement | Section |
|-------------|---------|
| Export all memory logs | §1, §5 |
| Export filtered (engine filter, no UI cap) | §1, §5 |
| Threadtime plain text | §5.2 |
| Save As dialog | §6 |
| Active device (incl. soft-disconnected) | §4.3 |
| No JSON / multi-device / clipboard | §1 out of scope |

---

## 11. Implementation notes for planning

- Files likely touched: `engine/src/device.rs` or `device_manager.rs`, `engine/src/websocket.rs`, `src/renderer/types/protocol.ts`, `src/renderer/App.tsx` (+ small export helper), `src/main/main.ts`, `src/main/preload.cjs`, tests under engine.
- Reuse existing `LogEntry` fields; avoid new crate deps.
- Align with CJS preload + `sandbox: false` already required for Electron launch.
