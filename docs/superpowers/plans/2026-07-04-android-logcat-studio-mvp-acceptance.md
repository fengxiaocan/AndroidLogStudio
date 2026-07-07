# Android Logcat Studio MVP Acceptance Checklist

- [ ] Windows app starts without Android Studio installed.
- [x] Engine prints `ALS_ENGINE_READY port=<port>` and serves `/ws` on localhost.
- [x] Renderer receives `device_list` and shows Mock Device in the device tab bar.
- [x] Engine resolves bundled ADB from `libs/<platform>/adb` or `libs/<platform>/adb.exe`.
- [x] ADB unavailable/no-device state falls back to Mock Device with WebSocket `adb_status.mode: mock_fallback` and a StatusBar fallback message. Verified at WebSocket level.
- [ ] Real connected devices stream `adb logcat -v threadtime` into the UI. `【未验证 / 未运行】` Not run because no usable bundled ADB / connected Android device was available during manual verification.
- [x] Renderer displays incoming mock log lines.
- [x] Visible log count remains at or below 500 by default.
- [x] Query Filter sends `set_filter` to the backend.
- [x] Current view search highlights matching text.
- [x] Full search sends `set_search` to the backend and receives `search_results`.
- [x] Recorder writes log files under `logs/<date>/<device>/`.
- [x] Status bar shows connection state and recorder path.
- [x] `cargo test -p als-engine` passes.
- [x] `npm run build` passes.
- [x] `npm run test:e2e` passes.
