# Android Logcat Studio (ALS) 设计文档

**版本：** 1.0  
**日期：** 2026-07-04  
**状态：** 设计已确认，待实现规划

---

## 1. 产品定位与核心目标

### 1.1 一句话目标

Android Logcat Studio（ALS）是一个跨平台、独立运行、无需 Android Studio、启动迅速、体验媲美新版 Logcat 的专业 Android 日志分析工具。

### 1.2 核心价值

- **随开随用**：不依赖重型 IDE，秒级启动
- **跨平台**：Windows 优先交付，架构和打包流程预留 Linux/macOS 支持
- **专业体验**：媲美 Android Studio 新版 Logcat 的过滤、搜索、颜色、统计能力
- **系统级调试**：后续逐步加入 Android Studio 没有的 Crash 模式、Bugreport、ADB 扩展等能力

### 1.3 目标用户

兼顾两类用户，按阶段服务：

1. Android 应用开发者（主要调试自己的 App）
2. Android 系统/ROM/驱动开发者（需要看 Framework、HAL、Kernel 日志）

### 1.4 关键设计决策

| 决策项 | 选择 | 原因 |
|---|---|---|
| 前端框架 | Electron + React/TS | 现代 UI 开发快，主题和布局灵活 |
| 后端引擎 | Rust | 性能、内存安全、二进制体积小 |
| 前后端通信 | 本地 WebSocket | 双向异步推送，解耦清晰 |
| 前端显示限制 | 最多 5000 行，默认 500 行 | 规避 Electron 大列表性能问题 |
| 后端存储策略 | 内存环形缓冲区 + 磁盘 Recorder | 实时性与持久化兼顾 |
| adb 分发方式 | 安装包自带 adb 二进制 | 开箱即用，无需配置 PATH |
| 第一平台 | Windows | 大多数 Android 开发者主力环境 |

---

## 2. 总体架构

### 2.1 架构图

```
┌─────────────────────────────────────────────────────────────┐
│                     Electron Frontend                        │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────────┐   │
│  │ Toolbar │ │ Device  │ │ Query   │ │   Log View      │   │
│  │         │ │ Tabs    │ │ Filter  │ │  (≤5000 rows)   │   │
│  └─────────┘ └─────────┘ └─────────┘ └─────────────────┘   │
│  ┌─────────┐ ┌─────────┐ ┌─────────────────────────────┐   │
│  │ Search  │ │Bookmarks│ │   Statistics / Tag Tree     │   │
│  └─────────┘ └─────────┘ └─────────────────────────────┘   │
└───────────────────────┬─────────────────────────────────────┘
                        │ WebSocket (JSON messages)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                      Rust Backend Engine                     │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │ Device      │ │ Log Parser  │ │ Filter Engine       │   │
│  │ Manager     │ │             │ │ (Query / Rules)     │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │ Memory Ring │ │ Recorder    │ │ Statistics Engine   │   │
│  │ Buffer      │ │ (Disk)      │ │                     │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│  ┌─────────────┐ ┌─────────────┐                           │
│  │ PID Cache   │ │ Color Engine│                           │
│  └─────────────┘ └─────────────┘                           │
└───────────────────────┬─────────────────────────────────────┘
                        │ spawn / read stdout
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                    Embedded Platform Tools                   │
│              tools/                                          │
│              ├── windows/   adb.exe + AdbWinApi.dll          │
│              ├── linux/     adb                              │
│              └── macos/     adb                              │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心数据流

```
adb logcat -v threadtime
        │
        ▼
Rust Reader Thread
        │
        ▼
Parser → LogEntry
        │
        ├──► Memory Ring Buffer
        │
        ├──► Recorder (async disk write)
        │
        ├──► Filter Engine
        │         │
        │         ▼
        │   Match current device filter
        │         │
        │         ▼
        ├──► WebSocket push to Electron
        │         │
        │         ▼
        │   Electron Log View (≤5000 visible rows)
        │
        └──► Statistics Engine (counts, top tags/packages)
```

### 2.3 关键原则

- **任何步骤不能阻塞 adb 读取**：adb 读取线程只做读取和投递
- **前端只负责显示，不参与重计算**：过滤、搜索、统计在后端完成
- **后端主动推送实时日志**：前端按需拉取历史/搜索/统计
- **多设备完全隔离**：每个设备独立进程、缓冲区、PID Cache、过滤、录制

---

## 3. 后端引擎设计（Rust）

### 3.1 模块职责

| 模块 | 职责 |
|---|---|
| `device_manager` | 枚举 adb 设备、启动/停止 `adb logcat` 子进程、管理多设备生命周期 |
| `log_reader` | 非阻塞读取 adb stdout，按行投递给解析器 |
| `log_parser` | 解析 `adb logcat -v threadtime` 输出为结构化 `LogEntry` |
| `pid_cache` | 维护 PID → 包名映射，按需刷新 |
| `ring_buffer` | 线程安全的内存环形缓冲区，保留最近 N 条日志 |
| `filter_engine` | 实时匹配 Include/Exclude 规则、Query 语法、Regex |
| `rule_engine` | 用户自定义规则（如自动隐藏某些 Tag） |
| `color_engine` | 根据 Level/Tag/Package 分配颜色 |
| `recorder` | 异步写入磁盘，支持按时间/大小切割、gzip 压缩 |
| `statistics` | 实时统计 Error/Warn 数量、Top Tag/Package/PID |
| `websocket_server` | 启动本地 WebSocket 服务，处理前端消息并推送事件 |

### 3.2 LogEntry 结构

```rust
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
    Assert,
    Unknown,
}

pub struct LogEntry {
    pub seq: u64,                       // 全局递增序号
    pub timestamp: u64,                 // Unix 毫秒
    pub date: String,                   // 日期
    pub time: String,                   // 时间
    pub pid: u32,
    pub tid: u32,
    pub level: LogLevel,
    pub tag: String,
    pub message: String,
    pub package_name: Option<String>,   // 通过 PID Cache 解析
    pub foreground: Option<String>,     // 前端颜色
    pub background: Option<String>,     // 背景颜色
    pub hidden: bool,                   // 被规则隐藏
    pub bookmarked: bool,
}
```

### 3.3 环形缓冲区

- 容量可配置，默认 **100 万条**
- 超过容量时覆盖最旧日志
- 每个设备独立一个 `DeviceContext`，包含独立缓冲区
- 使用 `crossbeam` 或自定义 ring buffer，保证无锁/少锁读取

### 3.4 多设备模型

每个连接的设备对应一个 `DeviceContext`：

```rust
pub struct DeviceContext {
    pub device_id: String,
    pub device_name: String,
    pub adb_process: Child,
    pub ring_buffer: Arc<RingBuffer<LogEntry>>,
    pub pid_cache: Arc<PidCache>,
    pub filter: Arc<FilterState>,
    pub recorder: Option<RecorderHandle>,
    pub statistics: Arc<Statistics>,
}
```

设备之间完全隔离，互不影响。

---

## 4. 前端设计（Electron + Web）

### 4.1 技术选择

- **Electron**：跨平台桌面容器
- **前端框架**：React + TypeScript（组件化、生态成熟）
- **状态管理**：Zustand 或 Jotai（轻量）
- **日志列表**：`react-window` 或 `react-virtuoso` 虚拟滚动
- **WebSocket 客户端**：原生 WebSocket API

### 4.2 视图布局

```
┌─────────────────────────────────────────────────────────────┐
│  [Device▼] [Start] [Stop] [Clear] [Settings] [Search Ctrl+F] │
├─────────────────────────────────────────────────────────────┤
│  Pixel 9 │ Redmi K70 │ RK3588 │ +                            │
├──────────┴──────────────────────────────────────────────────┤
│  Query Filter: package:launcher level:warn ...                │
├───────────────────────┬───────────────────────────────────────┤
│                       │                                       │
│   Log View            │   Statistics Panel                   │
│   (virtual list)      │   ────────────────                   │
│                       │   Errors: 234                          │
│   12:34:56.789 I Tag  │   Warnings: 823                        │
│   message line 1      │   Logs/s: 125                          │
│   message line 2      │   Memory: 18MB                         │
│                       │   Hidden: 450k                         │
│                       │                                       │
│                       │   Top Tags                             │
│                       │   ────────────────                     │
│                       │   SurfaceFlinger  12k                  │
│                       │   ActivityManager  8k                  │
│                       │                                       │
├───────────────────────┴───────────────────────────────────────┤
│  Status: Connected │ Recording ON │ 1,234,567 logs received   │
└─────────────────────────────────────────────────────────────┘
```

### 4.3 前端核心状态

```ts
interface AppState {
  devices: Device[];
  activeDeviceId: string | null;
  logs: LogEntry[];              // 当前设备可见日志，最多 5000
  filterQuery: string;           // Query Filter 输入
  searchQuery: string;           // Ctrl+F 搜索
  bookmarks: string[];           // 书签序号列表
  stats: Statistics;             // 当前设备统计
  settings: Settings;
}
```

### 4.4 前端行为

- 收到后端推送的新日志时，追加到列表，超出显示上限时移除最旧项
- 切换设备时，前端清空列表，后端推送该设备当前可见日志的快照
- 修改 Query Filter 时，发送给后端，后端重新计算匹配结果并推送
- 搜索分为两种模式：当前视图搜索由前端在当前可见日志中高亮匹配项；全量搜索由后端在内存缓冲区和必要的录制文件中执行，并返回匹配日志的 `seq` 列表和摘要

---

## 5. 通信协议（WebSocket JSON）

### 5.1 消息方向

| 方向 | 说明 |
|---|---|
| Frontend → Backend | 用户操作：连接设备、切换设备、修改过滤、搜索、书签 |
| Backend → Frontend | 事件推送：新日志、设备列表变化、统计更新、搜索结果 |

### 5.2 核心消息类型

```ts
// Frontend → Backend
interface ConnectDevice {
  type: "connect_device";
  deviceId: string;
}

interface DisconnectDevice {
  type: "disconnect_device";
  deviceId: string;
}

interface SetFilter {
  type: "set_filter";
  deviceId: string;
  query: string;
}

interface SetSearch {
  type: "set_search";
  deviceId: string;
  query: string;
  options: {
    regex: boolean;
    caseSensitive: boolean;
    wholeWord: boolean;
  };
}

interface GetHistory {
  type: "get_history";
  deviceId: string;
  beforeSeq: number;
  limit: number;
}

interface AddBookmark {
  type: "add_bookmark";
  deviceId: string;
  seq: number;
}

interface RemoveBookmark {
  type: "remove_bookmark";
  deviceId: string;
  seq: number;
}

interface GetStatistics {
  type: "get_statistics";
  deviceId: string;
}

// Backend → Frontend
interface NewLogs {
  type: "new_logs";
  deviceId: string;
  logs: LogEntry[];
}

interface DeviceList {
  type: "device_list";
  devices: Device[];
}

interface StatisticsUpdate {
  type: "statistics";
  deviceId: string;
  stats: Statistics;
}

interface SearchResults {
  type: "search_results";
  deviceId: string;
  matches: number[];  // 匹配的 seq 列表
}

interface Error {
  type: "error";
  message: string;
}
```

### 5.3 关键设计点

- 日志推送采用 **批量** 方式（如每 50ms 或每 100 条打包一次），减少 UI 刷新频率
- 切换设备时，后端先推送当前缓冲区中最新 500 条匹配过滤的日志作为快照
- 当前视图搜索由前端完成，用于快速高亮当前可见日志；全量搜索由后端完成，并通过 `search_results` 返回匹配结果

---

## 6. Query Filter 与规则系统

### 6.1 Query 语法（兼容 Android Studio）

```
package:com.demo.app
tag:SurfaceFlinger
level:error
pid:1234
text:Exception
is:crash
regex:ANR.*
```

### 6.2 组合与逻辑

```
package:launcher level:error            # 默认 AND
package:launcher OR package:systemui    # OR
-tag:OpenGLRenderer                     # NOT
```

### 6.3 规则引擎

用户可以保存常用规则，支持 Enable/Disable：

```json
{
  "name": "Hide system noise",
  "enabled": true,
  "rules": [
    { "type": "exclude", "tag": "ActivityTaskManager" },
    { "type": "exclude", "tag": "WindowManager" },
    { "type": "exclude", "tag": "InputDispatcher" }
  ]
}
```

### 6.4 Filter Profile

保存多套过滤方案，一键切换：

- Framework 调试
- HAL 调试
- 应用调试
- 自定义

---

## 7. 颜色、字体、列控制

### 7.1 颜色系统

按 **Log Level** 默认颜色：

| Level | 颜色 |
|---|---|
| V | 灰色 |
| D | 白色 |
| I | 绿色 |
| W | 黄色 |
| E | 红色 |
| A | 紫色 |

同时支持按 **Tag** 和按 **Package** 自定义颜色。颜色配置保存到 JSON 设置文件。

### 7.2 字体

- 支持等宽字体：Consolas、JetBrains Mono、Fira Code、Menlo、Monaco
- 字号范围：8 ~ 32
- 行高：紧凑 / 普通 / 宽松

### 7.3 列控制

可开关以下列：

- Date
- Time
- PID
- TID
- Tag
- Package
- Level

默认全部显示，用户可自定义紧凑布局。

---

## 8. Recorder（自动写盘）

### 8.1 行为

- 启动后默认开启录制
- 首次启动时提示 Recorder 默认开启，并允许用户关闭
- 状态栏持续显示录制状态、当前文件路径，并在磁盘空间不足时提示风险
- 每个设备独立目录：
  ```
  logs/
  └── 2026-07-04/
      └── <device-id-or-name>/
          ├── 12.log
          ├── 13.log
          └── 14.log
  ```
- 默认按小时切割文件
- 支持按天 / 按小时 / 按大小切换
- 支持 gzip 压缩

### 8.2 配置

```json
{
  "record": {
    "enabled": true,
    "path": "./logs",
    "rotation": "hourly",
    "maxSizeMB": 100,
    "compress": true
  }
}
```

### 8.3 实现

- Rust 后端异步写入，不阻塞日志读取
- 使用独立线程 + 有界通道，批量落盘
- 文件按本地时间命名，可配置 UTC

---

## 9. 搜索、书签、统计

### 9.1 搜索（Ctrl+F）

搜索分为两种模式：

- **当前视图搜索**：前端在当前可见日志中高亮匹配项，响应最快。
- **全量搜索**：前端提交搜索条件给 Rust 后端，后端在内存缓冲区和必要的录制文件中搜索，返回匹配日志的 `seq` 列表和摘要。

两种模式均支持：

- 普通文本搜索
- Regex 搜索
- 大小写敏感
- Whole Word
- 实时高亮匹配行

### 9.2 书签

- 第一期只支持 Manual 书签分类
- 右键菜单添加/移除书签
- 左侧 Bookmarks 面板展示 Manual 书签
- 点击书签快速跳转到对应日志
- Exception、ANR、Crash 自动分类随后续 Crash 模式实现

### 9.3 统计

右侧面板实时显示：

- Errors / Warnings 数量
- Logs/s
- 内存占用
- Hidden 日志数量
- Top Tag / Top Package / Top PID

---

## 10. 扩展功能（后续版本）

这些是 Android Studio 没有的系统级能力，建议后续版本逐步实现：

1. **Crash 模式**：自动检测 Java Crash、Native Crash、ANR、Tombstone，在时间轴上标记关键事件，并为书签提供 Exception、ANR、Crash 自动分类
2. **智能折叠**：连续重复日志自动折叠为 `×120`
3. **Tag 分组树**：左侧实时显示所有 Tag、数量、占比，可一键显示/隐藏
4. **时间轴定位**：拖动时间轴跳转到指定时间点
5. **ADB 扩展**：截图、录屏、安装 APK、重启、Recovery/Fastboot、查看进程、抓取 Bugreport

### 10.1 第一期范围

第一期 **不做** 以上扩展功能，先保证核心体验：

- 多设备管理
- 实时日志流
- Query Filter
- 搜索/书签
- 颜色/字体/列控制
- Recorder
- 统计面板

---

## 11. 错误处理与边界情况

| 场景 | 处理策略 |
|---|---|
| adb 未找到 | 提示用户检查 tools 目录，或自动检测内置 adb |
| 设备断开 | 后端自动重连，前端显示“Disconnected”状态 |
| adb logcat 进程崩溃 | 自动重启并标记日志流中断；优先使用 `adb logcat -T <last-timestamp>` 尝试从设备侧补拉缺口，失败时在前端显示日志缺口 |
| 前端 WebSocket 断开 | 后端保留状态，前端重连后推送快照 |
| 磁盘写满 | 暂停 Recorder，提示用户清理空间 |
| 内存缓冲区满 | 覆盖最旧日志，Recorder 仍有完整记录 |
| 解析失败 | 将原始行作为 `message`，`level=Unknown` 显示 |
| 超大单行日志 | 截断到配置长度（默认 10KB），完整内容可展开 |

### 关键原则

- adb 读取线程只做读取和投递
- 解析、过滤、录制、统计全部在独立线程/异步任务中
- 任何下游处理慢都不能阻塞上游读取

---

## 12. 测试策略

### 12.1 后端测试（Rust）

- **单元测试**：Parser、Filter Engine、Query 语法、Color Engine
- **集成测试**：启动 mock adb 进程，验证端到端数据流
- **性能测试**：以后端压力测试方式模拟峰值日志输入，例如 10 万行/秒，验证读取链路不阻塞、内存稳定、丢弃策略可观测
- **压力测试**：长时间运行 24 小时，验证内存无泄漏

### 12.2 前端测试（Electron + React）

- **组件测试**：Log View、Filter Input、Statistics Panel
- **E2E 测试**：启动应用，连接模拟设备，验证日志显示正常
- **性能测试**：5000 行日志滚动、搜索、切换设备流畅

### 12.3 跨平台测试

- Windows 优先
- Linux/macOS 后续通过 CI 覆盖

---

## 13. 项目目录结构（建议）

```
AndroidLogcatStudio/
├── Cargo.toml              # Rust workspace
├── package.json            # Electron + 前端
├── src/
│   └── main/               # Electron 主进程
├── renderer/
│   ├── src/                # React 前端源码
│   └── package.json
├── engine/
│   └── src/                # Rust 后端引擎
│       ├── main.rs
│       ├── device_manager.rs
│       ├── log_parser.rs
│       ├── filter_engine.rs
│       ├── ring_buffer.rs
│       ├── recorder.rs
│       ├── websocket_server.rs
│       └── ...
├── tools/
│   ├── windows/            # adb.exe + DLLs
│   ├── linux/              # adb
│   └── macos/              # adb
├── tests/
├── docs/
│   └── superpowers/specs/  # 设计文档
└── scripts/
    ├── build.ps1
    └── build.sh
```

---

## 14. 第一期验收标准

第一期完成时，ALS 应满足：

- Windows 上可独立启动，无需安装 Android Studio。
- 可使用内置 adb 枚举设备并启动 logcat。
- 单设备实时日志可显示，默认 500 行，最大 5000 行。
- Rust 后端持续保存最近 100 万条日志到内存缓冲区。
- Recorder 可按小时写入日志文件，并在状态栏显示录制状态。
- Query Filter 支持 `package`、`tag`、`level`、`pid`、`text` 基础字段。
- 当前视图搜索可高亮当前可见日志，全量搜索可由后端返回匹配结果。
- 前端断线重连后可恢复当前设备的最新日志快照。
- 5000 行日志滚动保持流畅。

---

## 15. 待确认事项

本设计文档已经过用户逐节确认，无待确认事项。下一步进入实现规划阶段。
