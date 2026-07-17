# Android Logcat Studio

**Android Logcat Studio (ALS)** 是一款专业的 Android 日志查看工具，致力于为开发者提供比 Android Studio Logcat 更强大、更流畅、更独立的体验。

无需安装 Android Studio，即可拥有媲美专业 Logcat 的工具。特别适合 Android 系统开发、ROM 开发、驱动开发以及需要处理海量日志的应用开发者使用。

## 项目简介

Android Logcat Studio 采用 **Electron + React** 前端 + **Rust** 高性能后端引擎的混合架构：

- 后端使用 Rust 实现高性能日志解析、1,000,000 条/设备的环形缓冲区、实时过滤与统计
- 前端提供现代化的用户界面，支持虚拟滚动，可流畅查看数十万甚至上百万条日志
- 内置 ADB 工具，无需用户单独安装

### 核心特性

- **高性能**：虚拟滚动 + Rust 后端，支持百万级日志实时查看
- **高级过滤**：支持包名、Tag（多值 OR 如 `tag1|tag2|tag3`、否定 `-tag:xxx`）、日志级别多选
- **大小写不敏感**：可切换是否忽略大小写匹配
- **包名自动解析**：通过 PID Cache 自动显示应用包名
- **全行着色**：可自定义各级别日志颜色，并支持整行高亮
- **多设备支持**：设备断连保护、日志隔离、快速切换
- **日志导出**：支持导出全部或过滤后的日志
- **中英双语界面**：支持中文和英文

### 技术栈

- 前端：Electron + React + TypeScript + Vite + Zustand + react-virtuoso
- 后端引擎：Rust（Axum WebSocket、自定义 Ring Buffer、过滤引擎）
- 通信：WebSocket（UI 与原生引擎进程）

当前版本 **v1.0.1** 已准备发布，提供 Linux .deb 安装包。

## Features

- **High Performance**: Virtual scrolling (react-virtuoso) + Rust backend ring buffer supports hundreds of thousands of logs smoothly.
- **Multi-Device Support**: Connect multiple devices, quick switching, soft disconnect handling, per-device log isolation.
- **Advanced Filtering**:
  - Package filter (supports `pkg1|pkg2`)
  - Tag filter with OR (`tag1 | tag2 | tag3`) and negation (`-tag:foo`)
  - Level checkboxes (multi-select OR)
  - Free text search
- **Package Name Enrichment**: Automatically resolves package names from PIDs using `adb shell ps`.
- **Customizable Appearance**:
  - Per-level colors with full-row highlighting
  - Toggleable columns
  - Dark theme optimized for logs
- **Log Export**: Export all or filtered logs from memory buffer.
- **Built-in ADB**: No external ADB installation required (bundled for Linux/macOS/Windows).
- **Recorder & Statistics**: Record logs to disk, real-time stats (errors, warnings, rate, memory).
- **Cross Platform**: Windows, macOS, Linux.
- **Bilingual UI**: English / Chinese.

## Tech Stack

- **Frontend**: Electron + React + TypeScript + Vite + Zustand + react-virtuoso
- **Backend Engine**: Rust (Axum WebSocket server, custom ring buffer, log parser, filter engine)
- **Communication**: WebSocket between UI and native engine process

## Installation

### Pre-built Releases (Recommended)

Download the latest `.deb` (or other formats) from GitHub Releases.

Install on Debian/Ubuntu:

```bash
sudo dpkg -i android-logcat-studio_1.0.1_amd64.deb
sudo apt-get install -f
```

### Build from Source

Requirements:
- Node.js 18+
- Rust (stable) + Cargo

```bash
git clone https://github.com/your-org/android-logcat-studio.git
cd android-logcat-studio
npm install

# Development
npm run dev:electron

# Release build
npm run build:release

# Package .deb
npm run package:deb
```

## Usage

1. Launch the app.
2. Connect Android devices (or use mock device for demo).
3. Use the top toolbar for package/tag/level filters.
4. Use the search bar for message search.
5. Open Settings (gear icon) to configure colors, visible columns, max rows, language.
6. Export visible or all logs using the export buttons.

The engine binary (`als-engine`) is automatically started as a child process and communicates over a local WebSocket.

## Configuration & Settings

- **Max Visible Rows**: Limits in-memory display (default 500, max 5000 for performance).
- **Level Colors**: Customize colors per log level (Verbose/Debug/Info/Warn/Error/Assert). Full row is tinted.
- **Columns**: Show/hide Time, PID, TID, Level, Package, Tag, Message.
- **Language**: English / 中文

## Architecture

```
adb logcat (or mock)
      │
   Engine (Rust)
   ├── Parser
   ├── Ring Buffer (1M entries per device)
   ├── Filter (package | tag | levels | negation | case-insensitive)
   ├── PID Cache
   ├── Statistics + Recorder
   └── WebSocket Server
      │
   Electron (React UI)
   ├── Device Management
   ├── QueryBar + Search
   ├── Virtual Log List
   └── Settings
```

## Building a Release

```bash
# Full release build (engine + frontend)
npm run build:release
```

## Packaging

### Linux .deb (for release)

```bash
# This will build release and produce dist/android-logcat-studio_1.0.0_amd64.deb
npm run package:deb
```

The resulting `.deb` can be installed with:

```bash
sudo dpkg -i dist/android-logcat-studio_1.0.0_amd64.deb
sudo apt-get install -f   # fix dependencies if needed
```

### Notes

- The app bundles platform-specific `adb` binaries under `libs/`.
- Engine binary (`als-engine`) is placed under `resources/engine/` in the packaged app.
- For v1.0.0, the package is built with the release-optimized Rust engine.

## v1.0.0 Release Highlights

- Initial stable release
- Advanced multi-tag and package filtering with OR (`|`) and negation (`-`)
- Case-insensitive search/filter toggle
- Full-row log level coloring
- PID-based package name enrichment
- Log export (all / filtered)
- Device disconnect resilience and per-device isolation
- Bilingual (EN/ZH) interface
- Bundled ADB tools

See CHANGELOG files in `docs/` for detailed history.

## Development

```bash
# Frontend only
npm run dev

# Run engine directly
npm run engine:run

# Tests
npm test
npm run engine:test

# E2E
npm run test:e2e
```

## License

MIT

## Contributing

Contributions welcome! The project follows a Rust + modern web frontend architecture for performance and maintainability.

---

**Android Logcat Studio** — Logs done right.
