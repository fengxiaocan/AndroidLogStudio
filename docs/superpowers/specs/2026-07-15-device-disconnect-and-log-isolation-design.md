# Device Disconnect & Per-Device Log Isolation Design

**Date:** 2026-07-15  
**Status:** Design approved, pending implementation plan  
**Scope:** Soft disconnect, device switch → engine snapshot, remove disconnected devices only, non-destructive device refresh

---

## 1. Goal

Fix three gaps in multi-device logcat handling:

1. **Device switch** — Show the selected device’s engine-buffered logs immediately via `log_snapshot`, then continue live `new_logs` for that device only.
2. **Soft disconnect** — When logcat exits or a device disappears from `adb devices`, mark `connected: false`, stop that device’s stream, keep history and list entry (labeled disconnected). No auto-reconnect except user Refresh / device reappearance.
3. **Remove** — Only disconnected devices may be removed: drop engine context + remove from `device_list`. Reappears after plug-in + Refresh.

**Out of scope (YAGNI)**

- Frontend `Map<deviceId, LogEntry[]>` full multi-device cache
- Auto-reconnect logcat without Refresh
- Removing online devices
- User-initiated disconnect of online devices (`disconnect_device` remains non-operational for that purpose)

---

## 2. Current state (gaps)

| Area | Today | Gap |
|---|---|---|
| Engine buffers | One `DeviceContext` + ring buffer per device; all online devices run logcat | `refresh()` rebuilds manager and **drops all buffers** |
| Child lifecycle | logcat children spawned | Exit not polled; `DeviceInfo.connected` never set `false` |
| UI switch | `setActiveDeviceId` clears frontend logs | No `connect_device` / snapshot request; list stays empty until new lines |
| `connect_device` | Deserialized | Stub: validate only |
| Remove device | — | Not implemented |

Baseline architecture (per-device context, WS tagged by `deviceId`) remains correct; this design fills lifecycle and switch wiring.

---

## 3. Architecture (Approach 1)

```
DeviceSelect change
  → setActiveDeviceId (clear UI logs)
  → client: connect_device { deviceId }
  → engine: log_snapshot + statistics + recorder_status
  → UI fills list; live new_logs only for active device

logcat child exit / Refresh reconciliation
  → DeviceManager: connected=false, stop child, keep DeviceContext
  → push device_list (and optional adb_status)

User Remove (disconnected only)
  → client: remove_device { deviceId }
  → engine: drop context + list entry; push device_list
  → if removed was active → select next / empty
```

**Source of truth for logs:** engine ring buffer per device. Frontend keeps a single `logs: LogEntry[]` for the active device only.

---

## 4. Protocol

### 4.1 Client messages

| Message | Behavior |
|---|---|
| `connect_device { deviceId }` | If unknown → `error`. Else send `log_snapshot` + `statistics` + `recorder_status` for that device. Does **not** start/stop logcat. Works for soft-disconnected devices (history only). |
| `remove_device { deviceId }` | **New.** If unknown → `error`. If `connected` → `error` (`device still connected`). Else drop context, remove from device list, push `device_list`. |
| `refresh_devices` | **Merge** refresh (see §5.3). Then `adb_status` + `device_list` + per-existing-device snapshot/stats/recorder (existing refresh state backfill). |
| `set_filter` / `set_search` / `get_statistics` | Unchanged; still device-scoped. |
| `disconnect_device` | Remain stub (validate only) this iteration; not the remove path. |

### 4.2 Server messages

| Message | Change |
|---|---|
| `device_list` | Actually set `DeviceInfo.connected` (`true` online, `false` soft-disconnected). |
| `log_snapshot`, `new_logs`, `statistics`, `recorder_status`, `error`, `adb_status` | Shapes unchanged. |

### 4.3 TypeScript delta

```ts
// ClientMessage
| { type: 'remove_device'; deviceId: string }

// connect_device already exists; engine implementation changes
// DeviceInfo.connected already exists; engine writes false on soft disconnect
```

### 4.4 Switch message sequence

```
UI: setActiveDeviceId(B) + empty logs
→ { type: 'connect_device', deviceId: 'B' }
→ existing effect: set_filter for B (may send a second snapshot; acceptable)
→ log_snapshot(B) + statistics + recorder_status
→ UI: logs = snapshot.slice(-visibleLimit)
→ only new_logs for B append
```

**Double snapshot:** `connect_device` and following `set_filter` may each push a snapshot. Content is consistent; correctness over optimization this iteration. Optional later: skip redundant snapshot on filter if filter unchanged.

---

## 5. Engine lifecycle

### 5.1 Device state model

| State | `connected` | logcat child | `DeviceContext` |
|---|---|---|---|
| Online | `true` | running | present, ingesting |
| Soft-disconnected | `false` | none | **kept** (snapshot readable) |
| Removed | — | none | **deleted** |

Mock devices: no logcat child; disconnect polling does not apply. Mock stays `connected: true` and is not removable under “disconnected only”.

### 5.2 Soft disconnect detection

**Primary:** each engine tick (same loop as `drain_pending_logs`), `try_wait` on each logcat child:

- Exited → remove from `logcat_children`, set that device’s `connected = false`, mark `devices_dirty`
- After tick, if dirty → push `device_list` (and optional `adb_status` message update)

**Secondary:** on Refresh, any ADB serial in manager missing from `adb devices -l` online set → soft disconnect (kill child if still present; keep context).

**No auto-reconnect** without user Refresh or serial reappearing in a merge refresh.

### 5.3 Refresh merge (replace full rebuild)

Do **not** `*self = from_scan_result(...)` when ADB devices already exist (that wipes buffers).

```
scan = online adb devices
for each serial in scan:
  - unknown: create DeviceContext + start logcat, connected=true
  - known + connected: if child dead, restart logcat; keep context
  - known + !connected: set connected=true, start logcat, keep context
for each ADB serial in manager not in scan:
  - soft disconnect
```

**ADB binary missing / list_devices error:** soft-disconnect all ADB devices; update `adb_status`; do **not** drop buffers.

**Mock fallback:** only when there has never been an ADB context (first start, no devices / no adb), same as today. Prefer not replacing an existing ADB device list with mock solely because refresh found zero online devices—soft-disconnect instead.

### 5.4 `connect_device`

```
if !has_device(id) → error
else → send_visible_snapshot + statistics + recorder_status
```

### 5.5 `remove_device`

```
if !has_device(id) → error "unknown device"
if connected → error "device still connected"
else drop context + remove from devices; ensure no child; push device_list
```

### 5.6 Resources

- Soft disconnect: kill + wait child (async path preferred)
- Engine shutdown: existing stop-all children
- PID cache lives with context; skip refresh while disconnected; resume when online again

### 5.7 WebSocket loop

- `poll_logcat_exits()` → dirty flag
- Dirty → `device_list` (+ optional `adb_status` e.g. `"1 online, 1 disconnected"`)
- `new_logs` only from devices still ingesting

---

## 6. Frontend behavior

### 6.1 Device switch

`handleDeviceChange(deviceId)`:

1. `setActiveDeviceId(deviceId)` — no-op if same; else set active + `emptyActiveDeviceState()`
2. Send `connect_device`
3. Existing `useEffect` re-sends `set_filter` (and search if needed)
4. Apply `log_snapshot` for active device; append only matching `new_logs`

Soft-disconnected devices remain selectable (history snapshot, no live stream).

### 6.2 `device_list` handling

```
if activeDeviceId still in list → keep active
else → first device or null

if active id changed → emptyActiveDeviceState + connect_device for new active (if any)
if same id and only connected flipped → update devices array only; do NOT clear logs
```

### 6.3 Disconnect UI

- DeviceSelect option label: online `Name · id (ADB)`; disconnected `Name · id (已断开)` / i18n
- Viewing a device that goes soft-disconnected: keep loaded logs; optional status hint
- Pause: still freezes only `new_logs`; snapshots still apply

### 6.4 Remove UI

- Control enabled only when active device exists and `connected === false`
- Recommended: button near device select (“移除设备” / “Remove device”)
- On click → `remove_device`
- After `device_list`: if removed was active, 6.2 selects next and requests snapshot; empty list → null active, empty logs

### 6.5 i18n keys

- `deviceDisconnected` — 已断开 / Disconnected  
- `removeDevice` — 移除设备 / Remove device  
- Engine `error.message` can surface for remove-while-online without dedicated toast type

---

## 7. Edge cases

| Scenario | Behavior |
|---|---|
| Switch to unknown id | Engine `error`; UI should not stick on invalid id |
| Remove while online | Engine error; list unchanged |
| Remove last device | Empty `device_list`; active null; empty logs; no forced mock from UI |
| Double snapshot | Acceptable |
| Refresh soft-disconnects active | Keep active + history; `connected` false |
| Refresh never removes | Only soft-disconnects missing serials |
| Same serial reconnects | Same context kept; new logcat appends; live stream resumes if still active |
| Multiple WS clients | Single-client assumption unchanged |

---

## 8. Testing

### 8.1 Rust

- Child exit → `connected=false`, context retained, snapshot non-empty if prior ingest
- `remove_device`: success when disconnected; error when connected or unknown
- Refresh merge: add / soft-disconnect / reconnect without wiping seq/buffer for surviving devices
- `connect_device` produces `log_snapshot` payload

### 8.2 Renderer / store

- Device change path sends `connect_device` (App or integration)
- `device_list` connected flip does not clear logs for same active id
- Removing active selects next / clears appropriately
- DeviceSelect shows disconnected label; remove disabled when online

### 8.3 Manual

- Two devices streaming; switch shows other device’s buffer immediately
- Unplug: label disconnected, history remains; Remove clears entry
- Replug + Refresh: device returns (context policy per §5.3)

### 8.4 Commands after implementation

```bash
cargo test -p als-engine
npm test   # or project unit test script
npm run build
npm run test:e2e
```

---

## 9. Implementation touchpoints (expected)

| Layer | Files (indicative) |
|---|---|
| Engine | `device_manager.rs` (poll exits, merge refresh, remove, connected flags), `websocket.rs` (`connect_device`, `remove_device`, dirty device_list) |
| Protocol | `src/renderer/types/protocol.ts`, Rust `ClientMessage` |
| Frontend | `App.tsx` (switch + remove send), `appStore.ts` (device_list rules), `DeviceSelect.tsx` / toolbar remove, `i18n` |
| Tests | `device_manager` tests, `websocket` deserialize/handler tests, `appStore.test.ts`, optional e2e |

---

## 10. Decisions log

| Decision | Choice |
|---|---|
| Switch fill strategy | Engine snapshot (A), not frontend multi-cache (B), not empty-until-live (C) |
| Disconnect UX | Soft disconnect; keep in list; keep history if viewing |
| Remove | Disconnected only; purge engine context + list |
| Approach | Protocol + engine lifecycle (1), not frontend-primary (2), not Refresh-only disconnect (3) |
| Auto-reconnect | No (Refresh / reappear only) |
| Double snapshot on switch | Accept for MVP |

---

## 11. Relation to prior specs

- Extends [2026-07-06 ADB Integration](./2026-07-06-adb-integration-design.md) (lifecycle incomplete there: “logcat exit → disconnected” was specified but not implemented).
- Aligns with multi-device isolation in [2026-07-04 ALS design](./2026-07-04-android-logcat-studio-design.md), but **soft disconnect without auto-reconnect** is intentional for this iteration (stricter than original §11 auto-reconnect matrix).
