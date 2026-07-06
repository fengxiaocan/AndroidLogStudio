# Android Logcat Studio ADB Integration Design

## Goal

Add real Android device support through bundled ADB binaries while keeping the current mock-device flow as a fallback. The first version should discover devices at startup, start logcat automatically for all online devices, and let the user manually refresh the device list.

## Bundled ADB layout

The app will use only bundled ADB binaries from this layout:

```text
libs/
  linux/adb
  macos/adb
  windows/adb.exe
```

The engine resolves the path by platform. It does not require Android Studio or a system Android SDK for the first version.

## Backend architecture

### `engine/src/adb.rs`

This module owns low-level ADB behavior:

- Resolve the platform-specific bundled ADB path.
- Check whether the binary exists and can be launched.
- Run `adb devices -l`.
- Parse `adb devices -l` output into `AdbDevice` records.
- Start `adb -s <serial> logcat -v threadtime` for a device.

The parser treats only `device` state entries as online. It ignores `offline`, `unauthorized`, `recovery`, and `sideload` entries for logcat startup.

### `engine/src/device_manager.rs`

This module owns device lifecycle:

- Scan once at engine startup.
- Start one logcat child process for every online ADB device.
- Keep one `DeviceContext` per active device.
- Fall back to Mock Device if ADB is missing, cannot start, or reports no online devices.
- Stop old logcat processes and rebuild device state on manual refresh.
- Kill all logcat child processes when the engine exits.

The manager exposes device list, ADB status, snapshots, statistics, recorder status, and search results to the WebSocket layer.

### `engine/src/websocket.rs`

The WebSocket layer should stay protocol-focused:

- Authenticate using the existing localhost token gate.
- Send `device_list` and `adb_status` after connection.
- Forward client actions to `DeviceManager`.
- Broadcast `new_logs`, `statistics`, and `recorder_status` from real or mock devices.
- Handle `refresh_devices` by asking `DeviceManager` to rescan and then sending fresh status and device list.

It should no longer own mock ticking directly once `DeviceManager` owns sources.

## Protocol changes

Add a client message:

```ts
{ type: 'refresh_devices' }
```

Add a server message:

```ts
{
  type: 'adb_status';
  available: boolean;
  mode: 'bundled' | 'mock_fallback';
  path: string | null;
  message: string;
}
```

Extend `DeviceInfo`:

```ts
interface DeviceInfo {
  deviceId: string;
  deviceName: string;
  connected: boolean;
  source: 'adb' | 'mock';
}
```

`deviceId` is the ADB serial for real devices and `mock-device` for fallback.

## Renderer changes

### Device tabs

Device tabs continue to show device name and ID. Real devices use ADB model/product data when available; otherwise they show the serial. Mock fallback shows `Mock Device` and `mock-device`.

### Refresh action

Add a `Refresh Devices` button near the device tabs or query controls. Clicking it sends `refresh_devices`. The first version does not need a loading state.

### Status bar

Add a concise ADB status segment. Examples:

```text
ADB: using bundled libs/linux/adb
ADB: missing libs/windows/adb.exe, using mock device
ADB: no online devices, using mock device
ADB: 2 devices connected
```

Existing connection, recorder path, warning, and visible-count display remain.

## Runtime behavior

### Startup with real devices

1. Electron starts the Rust engine with the existing token flow.
2. The engine resolves `libs/<platform>/adb`.
3. The engine runs `adb devices -l`.
4. For every online device, the engine starts `adb -s <serial> logcat -v threadtime`.
5. Each logcat stdout line goes through the existing parser, filter, recorder, statistics, and WebSocket flow.
6. The renderer receives real devices in `device_list` and log lines in `new_logs`.

### Startup without usable ADB

If ADB is missing, cannot start, or reports no online devices, the app still starts. The engine creates Mock Device and sends an `adb_status` message that explains the fallback.

### Manual refresh

When the renderer sends `refresh_devices`, the engine stops current logcat children, rescans devices, starts logcat for all online devices, and sends updated `adb_status` and `device_list` messages. If refresh finds no online devices, the manager switches to Mock Device.

## Error handling

- Missing ADB path: use Mock Device and report the missing path in `adb_status.message`.
- ADB launch failure: use Mock Device and report a concise failure message.
- No online devices: use Mock Device and report `no online devices`.
- Unauthorized or offline devices: ignore them for logcat startup.
- Logcat child exit: mark that device disconnected, stop sending logs for it, and expose a warning. The first version does not auto-reconnect.
- Unknown device IDs in client messages: keep the existing error-message behavior.

## Testing plan

### Rust tests

- Resolve the platform-specific `libs/<platform>/adb` path.
- Parse `adb devices -l` for one online device, multiple online devices, empty output, and ignored states.
- Fall back to Mock Device when ADB is missing.
- Create one `DeviceContext` per online device.
- Stop and rebuild device state on refresh.
- Route logcat lines into the correct device context.

### WebSocket tests

- Deserialize `refresh_devices`.
- Serialize `adb_status` with camelCase fields.
- Verify refresh sends updated `adb_status` and `device_list`.

### Renderer tests

- Render ADB status in the status bar.
- Send `refresh_devices` when the Refresh Devices button is clicked.
- Render both ADB and Mock device sources in device tabs.

### Manual verification

- With no `libs/<platform>/adb`, the app starts with Mock Device and an ADB fallback message.
- With bundled ADB and one online device, the app shows the real device and streams logcat automatically.
- With multiple online devices, the app starts logcat for all of them and allows tab switching.
- Refresh Devices rescans devices and updates the UI.
- Existing filtering, search, recording, and smoke tests still pass.

Required commands after implementation:

```bash
cargo test -p als-engine
npm run build
npm run test:e2e
```
