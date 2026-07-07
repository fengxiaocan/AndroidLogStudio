# Changelog 2026-07-06 ADB Integration

- Added bundled ADB path resolution under `libs/linux`, `libs/macos`, and `libs/windows`.
- Added startup device scanning with Mock Device fallback when ADB is unavailable or no online devices are connected.
- Added multi-device logcat process management through `DeviceManager`.
- Added `refresh_devices` and `adb_status` WebSocket protocol messages.
- Added renderer ADB status display and Refresh Devices action.
- Preserved existing filtering, search, recording, and smoke-test flows.
