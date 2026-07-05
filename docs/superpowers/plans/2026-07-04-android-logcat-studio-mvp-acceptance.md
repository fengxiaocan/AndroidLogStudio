# Android Logcat Studio MVP Acceptance Checklist

- [ ] Windows app starts without Android Studio installed.
- [x] Engine prints `ALS_ENGINE_READY port=<port>` and serves `/ws` on localhost.
- [x] Renderer receives `device_list` and shows Mock Device in the device tab bar.
- [x] Renderer displays incoming mock log lines.
- [x] Visible log count remains at or below 500 by default.
- [x] Query Filter sends `set_filter` to the backend.
- [x] Current view search highlights matching text.
- [x] Full search sends `set_search` to the backend and receives `search_results`.
- [x] Recorder writes log files under `logs/<date>/<device>/`.
- [ ] Status bar shows connection state and recorder path. `【未验证】` Connection state is visible, but runtime delivery of a concrete recorder path to the renderer was not confirmed.
- [x] `cargo test -p als-engine` passes.
- [x] `npm run build` passes.
- [x] `npm run test:e2e` passes.
