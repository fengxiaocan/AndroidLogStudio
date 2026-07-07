# ADB Library Layout

AndroidLogcatStudio resolves bundled ADB from canonical platform directories:

- Linux: `libs/linux/adb`
- macOS: `libs/macos/adb`
- Windows: `libs/windows/adb.exe` plus `libs/windows/AdbWinApi.dll` and `libs/windows/AdbWinUsbApi.dll`

The Rust engine does not look in `libs/mac` or `libs/win`. Those names may appear after extracting Android platform-tools archives manually, but they are legacy/local staging names for this project. Before packaging or running real-device verification, copy or rename them into the canonical directories above.

## Check the layout

Run the non-strict check when developing without committed binaries:

```bash
npm run adb:layout
```

Run the strict check before packaging a build that should include ADB:

```bash
npm run adb:layout:strict
```

The strict check fails if any canonical binary or Windows sidecar DLL is missing, or if Unix ADB binaries are not executable. The non-strict check allows missing binaries for local development, but still fails when a present Unix ADB binary is not executable.

## Current repository policy

The repository tracks the canonical `libs/<platform>/` directories with `.gitkeep` placeholders. It does not currently track Android platform-tools binaries. If binaries are added later, prefer a deliberate release strategy such as Git LFS or build-time artifact injection rather than accidentally committing local SDK downloads.
