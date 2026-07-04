# Android Logcat Studio MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows-first MVP of Android Logcat Studio with an Electron + React/TypeScript frontend, a Rust backend engine over local WebSocket, embedded adb discovery, real-time log display, basic filtering, search, recorder status, and acceptance tests.

**Architecture:** The Electron main process starts the Rust engine as a child process, reads the engine port from stdout, and passes the WebSocket URL to the renderer. The Rust engine owns adb communication, parsing, filtering, buffering, recorder writes, statistics, and WebSocket events. The React renderer keeps only the currently visible log window in memory and delegates full-log work to the backend.

**Tech Stack:** Rust stable, Tokio, Axum WebSocket, Serde, Regex, Chrono, Flate2, Electron, React, TypeScript, Vite, Zustand, Vitest, Playwright.

---

## File Structure Map

Create or modify these files:

```text
AndroidLogcatStudio/
├── Cargo.toml
├── package.json
├── tsconfig.json
├── vite.config.ts
├── index.html
├── src/
│   ├── main/
│   │   ├── main.ts
│   │   └── preload.ts
│   └── renderer/
│       ├── App.tsx
│       ├── main.tsx
│       ├── styles.css
│       ├── api/
│       │   └── engineClient.ts
│       ├── components/
│       │   ├── DeviceTabs.tsx
│       │   ├── LogView.tsx
│       │   ├── QueryBar.tsx
│       │   ├── SearchBar.tsx
│       │   ├── StatusBar.tsx
│       │   └── StatsPanel.tsx
│       ├── state/
│       │   └── appStore.ts
│       └── types/
│           └── protocol.ts
├── engine/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── adb.rs
│       ├── device.rs
│       ├── filter.rs
│       ├── log_entry.rs
│       ├── parser.rs
│       ├── recorder.rs
│       ├── ring_buffer.rs
│       ├── statistics.rs
│       └── websocket.rs
├── tests/
│   ├── fixtures/
│   │   ├── mock-adb.js
│   │   └── sample-threadtime.log
│   └── e2e/
│       └── app.spec.ts
├── tools/
│   ├── windows/.gitkeep
│   ├── linux/.gitkeep
│   └── macos/.gitkeep
└── docs/
    └── superpowers/
        ├── specs/2026-07-04-android-logcat-studio-design.md
        └── plans/2026-07-04-android-logcat-studio-mvp.md
```

Responsibility boundaries:

- `engine/src/parser.rs`: parse one raw `adb logcat -v threadtime` line into `LogEntry`.
- `engine/src/filter.rs`: parse and evaluate basic query terms: `package`, `tag`, `level`, `pid`, and `text`.
- `engine/src/ring_buffer.rs`: hold latest N `LogEntry` values per device.
- `engine/src/recorder.rs`: write raw log lines to hourly files without blocking readers.
- `engine/src/websocket.rs`: expose typed JSON protocol over local WebSocket.
- `src/renderer/api/engineClient.ts`: single frontend gateway for WebSocket messages.
- `src/renderer/state/appStore.ts`: single source of frontend UI state.
- `src/renderer/components/*`: presentational UI components with minimal logic.

Commit note: this directory is currently not a git repository. Commit steps below are active only after running `git init` or moving the project into a repository.

---

## Task 1: Scaffold the Rust + Electron Workspace

**Files:**
- Create: `Cargo.toml`
- Create: `engine/Cargo.toml`
- Create: `engine/src/main.rs`
- Create: `package.json`
- Create: `tsconfig.json`
- Create: `vite.config.ts`
- Create: `index.html`
- Create: `src/main/main.ts`
- Create: `src/main/preload.ts`
- Create: `src/renderer/main.tsx`
- Create: `src/renderer/App.tsx`
- Create: `src/renderer/styles.css`
- Create: `tools/windows/.gitkeep`
- Create: `tools/linux/.gitkeep`
- Create: `tools/macos/.gitkeep`

- [ ] **Step 1: Create the Rust workspace manifest**

Create `Cargo.toml`:

```toml
[workspace]
members = ["engine"]
resolver = "2"
```

- [ ] **Step 2: Create the Rust engine manifest**

Create `engine/Cargo.toml`:

```toml
[package]
name = "als-engine"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
axum = { version = "0.7", features = ["ws"] }
chrono = { version = "0.4", features = ["serde"] }
flate2 = "1"
futures = "0.3"
regex = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
tower-http = { version = "0.5", features = ["cors"] }
uuid = { version = "1", features = ["v4", "serde"] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Add the first engine executable**

Create `engine/src/main.rs`:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ALS_ENGINE_READY port=0");
    Ok(())
}
```

- [ ] **Step 4: Create the frontend package manifest**

Create `package.json`:

```json
{
  "name": "android-logcat-studio",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "main": "dist/main/main.js",
  "scripts": {
    "dev": "vite --host 127.0.0.1",
    "dev:electron": "electron .",
    "build": "tsc --noEmit && vite build",
    "test": "vitest run",
    "test:e2e": "playwright test",
    "engine:test": "cargo test -p als-engine",
    "engine:run": "cargo run -p als-engine"
  },
  "dependencies": {
    "@vitejs/plugin-react": "latest",
    "electron": "latest",
    "react": "latest",
    "react-dom": "latest",
    "react-virtuoso": "latest",
    "zustand": "latest"
  },
  "devDependencies": {
    "@playwright/test": "latest",
    "@testing-library/jest-dom": "latest",
    "@testing-library/react": "latest",
    "@types/node": "latest",
    "@types/react": "latest",
    "@types/react-dom": "latest",
    "typescript": "latest",
    "vite": "latest",
    "vitest": "latest"
  }
}
```

- [ ] **Step 5: Add TypeScript configuration**

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["DOM", "DOM.Iterable", "ES2022"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Node",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx"
  },
  "include": ["src", "vite.config.ts"]
}
```

- [ ] **Step 6: Add Vite configuration**

Create `vite.config.ts`:

```ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  root: '.',
  build: {
    outDir: 'dist/renderer',
    emptyOutDir: true,
  },
  server: {
    host: '127.0.0.1',
    port: 5173,
  },
});
```

- [ ] **Step 7: Add the HTML shell**

Create `index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Android Logcat Studio</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/renderer/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 8: Add the Electron main process**

Create `src/main/main.ts`:

```ts
import { app, BrowserWindow } from 'electron';
import path from 'node:path';

async function createWindow() {
  const win = new BrowserWindow({
    width: 1400,
    height: 900,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (process.env.VITE_DEV_SERVER_URL) {
    await win.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    await win.loadFile(path.join(__dirname, '../renderer/index.html'));
  }
}

app.whenReady().then(createWindow);

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});
```

- [ ] **Step 9: Add a safe preload bridge**

Create `src/main/preload.ts`:

```ts
import { contextBridge } from 'electron';

contextBridge.exposeInMainWorld('als', {
  version: '0.1.0',
});
```

- [ ] **Step 10: Add the renderer entry point**

Create `src/renderer/main.tsx`:

```tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import './styles.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 11: Add the first React app shell**

Create `src/renderer/App.tsx`:

```tsx
export function App() {
  return (
    <main className="app-shell">
      <header className="toolbar">Android Logcat Studio</header>
      <section className="empty-state">Engine connection pending</section>
    </main>
  );
}
```

- [ ] **Step 12: Add base styling**

Create `src/renderer/styles.css`:

```css
:root {
  color-scheme: dark;
  font-family: Inter, Segoe UI, system-ui, sans-serif;
  background: #101318;
  color: #e6edf3;
}

body {
  margin: 0;
}

.app-shell {
  min-height: 100vh;
  display: grid;
  grid-template-rows: 48px 1fr;
}

.toolbar {
  display: flex;
  align-items: center;
  padding: 0 16px;
  border-bottom: 1px solid #263140;
  background: #151a21;
  font-weight: 600;
}

.empty-state {
  display: grid;
  place-items: center;
  color: #8b949e;
}
```

- [ ] **Step 13: Create embedded adb directories**

Create these empty files:

```text
tools/windows/.gitkeep
tools/linux/.gitkeep
tools/macos/.gitkeep
```

- [ ] **Step 14: Run initial build checks**

Run:

```bash
cargo test -p als-engine
npm install
npm run build
```

Expected:

```text
cargo test: 0 tests, finished successfully
npm install: dependencies installed
npm run build: TypeScript and Vite complete without errors
```

- [ ] **Step 15: Commit scaffold if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add Cargo.toml engine package.json tsconfig.json vite.config.ts index.html src tools && git commit -m "chore: scaffold ALS workspace"
```

Expected in a git repository:

```text
[branch commit] chore: scaffold ALS workspace
```

Expected outside git:

```text
fatal: not a git repository
```

---

## Task 2: Define Shared Protocol Types

**Files:**
- Create: `engine/src/log_entry.rs`
- Modify: `engine/src/main.rs`
- Create: `src/renderer/types/protocol.ts`

- [ ] **Step 1: Write the Rust protocol model**

Create `engine/src/log_entry.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
    Assert,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub seq: u64,
    pub timestamp: u64,
    pub date: String,
    pub time: String,
    pub pid: u32,
    pub tid: u32,
    pub level: LogLevel,
    pub tag: String,
    pub message: String,
    pub package_name: Option<String>,
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub hidden: bool,
    pub bookmarked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub connected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsSnapshot {
    pub errors: u64,
    pub warnings: u64,
    pub logs_per_second: u64,
    pub memory_bytes: u64,
    pub hidden: u64,
}
```

- [ ] **Step 2: Export the Rust module**

Replace `engine/src/main.rs` with:

```rust
mod log_entry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ALS_ENGINE_READY port=0");
    Ok(())
}
```

- [ ] **Step 3: Add TypeScript protocol types**

Create `src/renderer/types/protocol.ts`:

```ts
export type LogLevel = 'verbose' | 'debug' | 'info' | 'warn' | 'error' | 'assert' | 'unknown';

export interface LogEntry {
  seq: number;
  timestamp: number;
  date: string;
  time: string;
  pid: number;
  tid: number;
  level: LogLevel;
  tag: string;
  message: string;
  packageName: string | null;
  foreground: string | null;
  background: string | null;
  hidden: boolean;
  bookmarked: boolean;
}

export interface DeviceInfo {
  deviceId: string;
  deviceName: string;
  connected: boolean;
}

export interface StatisticsSnapshot {
  errors: number;
  warnings: number;
  logsPerSecond: number;
  memoryBytes: number;
  hidden: number;
}

export type ClientMessage =
  | { type: 'connect_device'; deviceId: string }
  | { type: 'disconnect_device'; deviceId: string }
  | { type: 'set_filter'; deviceId: string; query: string }
  | { type: 'set_search'; deviceId: string; query: string; options: SearchOptions }
  | { type: 'get_history'; deviceId: string; beforeSeq: number; limit: number }
  | { type: 'add_bookmark'; deviceId: string; seq: number }
  | { type: 'remove_bookmark'; deviceId: string; seq: number }
  | { type: 'get_statistics'; deviceId: string };

export interface SearchOptions {
  regex: boolean;
  caseSensitive: boolean;
  wholeWord: boolean;
}

export type ServerMessage =
  | { type: 'new_logs'; deviceId: string; logs: LogEntry[] }
  | { type: 'device_list'; devices: DeviceInfo[] }
  | { type: 'statistics'; deviceId: string; stats: StatisticsSnapshot }
  | { type: 'search_results'; deviceId: string; matches: number[] }
  | { type: 'recorder_status'; deviceId: string; enabled: boolean; path: string | null; warning: string | null }
  | { type: 'error'; message: string };
```

- [ ] **Step 4: Run type and Rust checks**

Run:

```bash
cargo test -p als-engine
npm run build
```

Expected:

```text
Rust compiles successfully
TypeScript compiles successfully
```

- [ ] **Step 5: Commit protocol types if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add engine/src/log_entry.rs engine/src/main.rs src/renderer/types/protocol.ts && git commit -m "feat: define ALS protocol types"
```

---

## Task 3: Implement Logcat Threadtime Parser

**Files:**
- Create: `engine/src/parser.rs`
- Modify: `engine/src/main.rs`
- Test: `engine/src/parser.rs`

- [ ] **Step 1: Write parser tests first**

Create `engine/src/parser.rs` with tests and a stub:

```rust
use crate::log_entry::{LogEntry, LogLevel};

pub fn parse_threadtime_line(seq: u64, line: &str) -> Option<LogEntry> {
    let _ = (seq, line);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_threadtime_line() {
        let line = "07-04 12:34:56.789  1234  5678 I ActivityManager: Start proc com.example";
        let entry = parse_threadtime_line(42, line).expect("line should parse");

        assert_eq!(entry.seq, 42);
        assert_eq!(entry.date, "07-04");
        assert_eq!(entry.time, "12:34:56.789");
        assert_eq!(entry.pid, 1234);
        assert_eq!(entry.tid, 5678);
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.tag, "ActivityManager");
        assert_eq!(entry.message, "Start proc com.example");
        assert_eq!(entry.package_name, None);
        assert!(!entry.hidden);
        assert!(!entry.bookmarked);
    }

    #[test]
    fn returns_unknown_entry_for_unparseable_line() {
        let entry = parse_threadtime_line(7, "raw line without threadtime fields").expect("raw line preserved");
        assert_eq!(entry.level, LogLevel::Unknown);
        assert_eq!(entry.message, "raw line without threadtime fields");
        assert_eq!(entry.tag, "");
    }

    #[test]
    fn truncates_large_message_to_ten_kib() {
        let message = "x".repeat(11 * 1024);
        let line = format!("07-04 12:34:56.789  1234  5678 E Tag: {message}");
        let entry = parse_threadtime_line(1, &line).expect("line should parse");
        assert_eq!(entry.message.len(), 10 * 1024);
    }
}
```

- [ ] **Step 2: Export the parser module**

Replace `engine/src/main.rs` with:

```rust
mod log_entry;
mod parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ALS_ENGINE_READY port=0");
    Ok(())
}
```

- [ ] **Step 3: Run parser tests and verify failure**

Run:

```bash
cargo test -p als-engine parser -- --nocapture
```

Expected:

```text
parses_standard_threadtime_line ... FAILED
```

- [ ] **Step 4: Implement parser logic**

Replace `engine/src/parser.rs` with:

```rust
use crate::log_entry::{LogEntry, LogLevel};
use regex::Regex;
use std::sync::OnceLock;

const MAX_MESSAGE_BYTES: usize = 10 * 1024;

fn threadtime_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^(?P<date>\d{2}-\d{2})\s+(?P<time>\d{2}:\d{2}:\d{2}\.\d{3})\s+(?P<pid>\d+)\s+(?P<tid>\d+)\s+(?P<level>[VDIWEAF])\s+(?P<tag>[^:]+):\s?(?P<message>.*)$")
            .expect("threadtime regex must compile")
    })
}

pub fn parse_threadtime_line(seq: u64, line: &str) -> Option<LogEntry> {
    let captures = threadtime_regex().captures(line);

    match captures {
        Some(caps) => {
            let message = truncate_message(caps.name("message")?.as_str());
            Some(LogEntry {
                seq,
                timestamp: 0,
                date: caps.name("date")?.as_str().to_string(),
                time: caps.name("time")?.as_str().to_string(),
                pid: caps.name("pid")?.as_str().parse().ok()?,
                tid: caps.name("tid")?.as_str().parse().ok()?,
                level: parse_level(caps.name("level")?.as_str()),
                tag: caps.name("tag")?.as_str().trim().to_string(),
                message,
                package_name: None,
                foreground: None,
                background: None,
                hidden: false,
                bookmarked: false,
            })
        }
        None => Some(LogEntry {
            seq,
            timestamp: 0,
            date: String::new(),
            time: String::new(),
            pid: 0,
            tid: 0,
            level: LogLevel::Unknown,
            tag: String::new(),
            message: truncate_message(line),
            package_name: None,
            foreground: None,
            background: None,
            hidden: false,
            bookmarked: false,
        }),
    }
}

fn parse_level(level: &str) -> LogLevel {
    match level {
        "V" => LogLevel::Verbose,
        "D" => LogLevel::Debug,
        "I" => LogLevel::Info,
        "W" => LogLevel::Warn,
        "E" => LogLevel::Error,
        "A" | "F" => LogLevel::Assert,
        _ => LogLevel::Unknown,
    }
}

fn truncate_message(message: &str) -> String {
    if message.len() <= MAX_MESSAGE_BYTES {
        return message.to_string();
    }

    let mut end = MAX_MESSAGE_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_threadtime_line() {
        let line = "07-04 12:34:56.789  1234  5678 I ActivityManager: Start proc com.example";
        let entry = parse_threadtime_line(42, line).expect("line should parse");

        assert_eq!(entry.seq, 42);
        assert_eq!(entry.date, "07-04");
        assert_eq!(entry.time, "12:34:56.789");
        assert_eq!(entry.pid, 1234);
        assert_eq!(entry.tid, 5678);
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.tag, "ActivityManager");
        assert_eq!(entry.message, "Start proc com.example");
        assert_eq!(entry.package_name, None);
        assert!(!entry.hidden);
        assert!(!entry.bookmarked);
    }

    #[test]
    fn returns_unknown_entry_for_unparseable_line() {
        let entry = parse_threadtime_line(7, "raw line without threadtime fields").expect("raw line preserved");
        assert_eq!(entry.level, LogLevel::Unknown);
        assert_eq!(entry.message, "raw line without threadtime fields");
        assert_eq!(entry.tag, "");
    }

    #[test]
    fn truncates_large_message_to_ten_kib() {
        let message = "x".repeat(11 * 1024);
        let line = format!("07-04 12:34:56.789  1234  5678 E Tag: {message}");
        let entry = parse_threadtime_line(1, &line).expect("line should parse");
        assert_eq!(entry.message.len(), 10 * 1024);
    }
}
```

- [ ] **Step 5: Run parser tests and verify pass**

Run:

```bash
cargo test -p als-engine parser -- --nocapture
```

Expected:

```text
test result: ok. 3 passed
```

- [ ] **Step 6: Commit parser if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add engine/src/parser.rs engine/src/main.rs && git commit -m "feat: parse threadtime logcat lines"
```

---

## Task 4: Implement Ring Buffer, Filter Engine, and Statistics

**Files:**
- Create: `engine/src/ring_buffer.rs`
- Create: `engine/src/filter.rs`
- Create: `engine/src/statistics.rs`
- Modify: `engine/src/main.rs`

- [ ] **Step 1: Add ring buffer tests and implementation**

Create `engine/src/ring_buffer.rs`:

```rust
use std::collections::VecDeque;

#[derive(Debug)]
pub struct RingBuffer<T> {
    capacity: usize,
    values: VecDeque<T>,
}

impl<T: Clone> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "ring buffer capacity must be greater than zero");
        Self { capacity, values: VecDeque::with_capacity(capacity) }
    }

    pub fn push(&mut self, value: T) {
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    pub fn latest(&self, limit: usize) -> Vec<T> {
        let start = self.values.len().saturating_sub(limit);
        self.values.iter().skip(start).cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_latest_values_when_full() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        buffer.push(4);

        assert_eq!(buffer.latest(10), vec![2, 3, 4]);
    }

    #[test]
    fn latest_respects_limit() {
        let mut buffer = RingBuffer::new(5);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        assert_eq!(buffer.latest(2), vec![2, 3]);
    }
}
```

- [ ] **Step 2: Add filter tests and implementation**

Create `engine/src/filter.rs`:

```rust
use crate::log_entry::{LogEntry, LogLevel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterTerm {
    Package(String),
    Tag(String),
    Level(LogLevel),
    Pid(u32),
    Text(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterQuery {
    terms: Vec<FilterTerm>,
}

impl FilterQuery {
    pub fn parse(input: &str) -> Self {
        let terms = input
            .split_whitespace()
            .filter_map(parse_term)
            .collect();
        Self { terms }
    }

    pub fn matches(&self, entry: &LogEntry) -> bool {
        self.terms.iter().all(|term| match term {
            FilterTerm::Package(value) => entry.package_name.as_deref().unwrap_or("").contains(value),
            FilterTerm::Tag(value) => entry.tag.contains(value),
            FilterTerm::Level(value) => &entry.level == value,
            FilterTerm::Pid(value) => entry.pid == *value,
            FilterTerm::Text(value) => entry.message.contains(value),
        })
    }
}

fn parse_term(raw: &str) -> Option<FilterTerm> {
    let (key, value) = raw.split_once(':')?;
    match key {
        "package" => Some(FilterTerm::Package(value.to_string())),
        "tag" => Some(FilterTerm::Tag(value.to_string())),
        "level" => parse_level(value).map(FilterTerm::Level),
        "pid" => value.parse().ok().map(FilterTerm::Pid),
        "text" => Some(FilterTerm::Text(value.to_string())),
        _ => None,
    }
}

fn parse_level(value: &str) -> Option<LogLevel> {
    match value.to_ascii_lowercase().as_str() {
        "v" | "verbose" => Some(LogLevel::Verbose),
        "d" | "debug" => Some(LogLevel::Debug),
        "i" | "info" => Some(LogLevel::Info),
        "w" | "warn" => Some(LogLevel::Warn),
        "e" | "error" => Some(LogLevel::Error),
        "a" | "assert" => Some(LogLevel::Assert),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> LogEntry {
        LogEntry {
            seq: 1,
            timestamp: 0,
            date: "07-04".to_string(),
            time: "12:00:00.000".to_string(),
            pid: 1234,
            tid: 5678,
            level: LogLevel::Error,
            tag: "ActivityManager".to_string(),
            message: "Process crashed".to_string(),
            package_name: Some("com.example".to_string()),
            foreground: None,
            background: None,
            hidden: false,
            bookmarked: false,
        }
    }

    #[test]
    fn matches_basic_and_query() {
        let query = FilterQuery::parse("package:example level:error text:crashed");
        assert!(query.matches(&entry()));
    }

    #[test]
    fn rejects_non_matching_query() {
        let query = FilterQuery::parse("tag:SurfaceFlinger");
        assert!(!query.matches(&entry()));
    }
}
```

- [ ] **Step 3: Add statistics tests and implementation**

Create `engine/src/statistics.rs`:

```rust
use crate::log_entry::{LogEntry, LogLevel, StatisticsSnapshot};

#[derive(Debug, Default)]
pub struct Statistics {
    errors: u64,
    warnings: u64,
    hidden: u64,
    total: u64,
}

impl Statistics {
    pub fn observe(&mut self, entry: &LogEntry) {
        self.total += 1;
        if entry.hidden {
            self.hidden += 1;
        }
        match entry.level {
            LogLevel::Error | LogLevel::Assert => self.errors += 1,
            LogLevel::Warn => self.warnings += 1,
            _ => {}
        }
    }

    pub fn snapshot(&self) -> StatisticsSnapshot {
        StatisticsSnapshot {
            errors: self.errors,
            warnings: self.warnings,
            logs_per_second: 0,
            memory_bytes: self.total * 256,
            hidden: self.hidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(level: LogLevel) -> LogEntry {
        LogEntry {
            seq: 1,
            timestamp: 0,
            date: String::new(),
            time: String::new(),
            pid: 0,
            tid: 0,
            level,
            tag: String::new(),
            message: String::new(),
            package_name: None,
            foreground: None,
            background: None,
            hidden: false,
            bookmarked: false,
        }
    }

    #[test]
    fn counts_errors_and_warnings() {
        let mut stats = Statistics::default();
        stats.observe(&entry(LogLevel::Error));
        stats.observe(&entry(LogLevel::Warn));

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.errors, 1);
        assert_eq!(snapshot.warnings, 1);
    }
}
```

- [ ] **Step 4: Export modules**

Replace `engine/src/main.rs` with:

```rust
mod filter;
mod log_entry;
mod parser;
mod ring_buffer;
mod statistics;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ALS_ENGINE_READY port=0");
    Ok(())
}
```

- [ ] **Step 5: Run backend tests**

Run:

```bash
cargo test -p als-engine
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit backend core utilities if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add engine/src && git commit -m "feat: add backend log buffer filtering and stats"
```

---

## Task 5: Implement Recorder with Hourly File Rotation

**Files:**
- Create: `engine/src/recorder.rs`
- Modify: `engine/src/main.rs`

- [ ] **Step 1: Write recorder tests and implementation**

Create `engine/src/recorder.rs`:

```rust
use anyhow::Context;
use chrono::{Local, Timelike};
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RecorderConfig {
    pub enabled: bool,
    pub root: PathBuf,
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecorderStatus {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub warning: Option<String>,
}

pub struct Recorder {
    config: RecorderConfig,
    current_path: Option<PathBuf>,
}

impl Recorder {
    pub fn new(config: RecorderConfig) -> Self {
        Self { config, current_path: None }
    }

    pub fn write_line(&mut self, line: &str) -> anyhow::Result<RecorderStatus> {
        if !self.config.enabled {
            return Ok(RecorderStatus { enabled: false, path: None, warning: None });
        }

        let path = self.current_hour_path();
        if let Some(parent) = path.parent() {
            create_dir_all(parent).with_context(|| format!("create recorder directory {}", parent.display()))?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open recorder file {}", path.display()))?;
        writeln!(file, "{line}").with_context(|| format!("write recorder file {}", path.display()))?;
        self.current_path = Some(path.clone());

        Ok(RecorderStatus { enabled: true, path: Some(path), warning: None })
    }

    fn current_hour_path(&self) -> PathBuf {
        let now = Local::now();
        let day = now.format("%Y-%m-%d").to_string();
        let hour = format!("{:02}.log", now.hour());
        sanitize_path(&self.config.root, &day, &self.config.device_name, &hour)
    }
}

fn sanitize_path(root: &Path, day: &str, device: &str, file: &str) -> PathBuf {
    let safe_device = device
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
        .collect::<String>();
    root.join(day).join(safe_device).join(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_log_line_to_device_directory() {
        let dir = tempdir().expect("tempdir");
        let mut recorder = Recorder::new(RecorderConfig {
            enabled: true,
            root: dir.path().to_path_buf(),
            device_name: "Pixel 9".to_string(),
        });

        let status = recorder.write_line("hello log").expect("write succeeds");

        assert!(status.enabled);
        let path = status.path.expect("path set");
        assert!(path.display().to_string().contains("Pixel_9"));
        let content = std::fs::read_to_string(path).expect("read log");
        assert!(content.contains("hello log"));
    }

    #[test]
    fn disabled_recorder_does_not_write() {
        let dir = tempdir().expect("tempdir");
        let mut recorder = Recorder::new(RecorderConfig {
            enabled: false,
            root: dir.path().to_path_buf(),
            device_name: "Pixel".to_string(),
        });

        let status = recorder.write_line("hello log").expect("disabled succeeds");

        assert!(!status.enabled);
        assert!(status.path.is_none());
    }
}
```

- [ ] **Step 2: Export recorder module**

Replace `engine/src/main.rs` with:

```rust
mod filter;
mod log_entry;
mod parser;
mod recorder;
mod ring_buffer;
mod statistics;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ALS_ENGINE_READY port=0");
    Ok(())
}
```

- [ ] **Step 3: Run recorder tests**

Run:

```bash
cargo test -p als-engine recorder -- --nocapture
```

Expected:

```text
test result: ok. 2 passed
```

- [ ] **Step 4: Commit recorder if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add engine/src/recorder.rs engine/src/main.rs && git commit -m "feat: add hourly log recorder"
```

---

## Task 6: Implement Device and adb Process Layer

**Files:**
- Create: `engine/src/adb.rs`
- Create: `engine/src/device.rs`
- Modify: `engine/src/main.rs`
- Create: `tests/fixtures/sample-threadtime.log`

- [ ] **Step 1: Add a sample log fixture**

Create `tests/fixtures/sample-threadtime.log`:

```text
07-04 12:34:56.789  1234  5678 I ActivityManager: Start proc com.example
07-04 12:34:57.000  1234  5678 W ActivityManager: Slow operation
07-04 12:34:58.111  4321  8765 E AndroidRuntime: FATAL EXCEPTION: main
```

- [ ] **Step 2: Implement adb path resolution**

Create `engine/src/adb.rs`:

```rust
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AdbPaths {
    pub adb: PathBuf,
}

pub fn resolve_adb_path(project_root: &Path) -> AdbPaths {
    let relative = if cfg!(target_os = "windows") {
        PathBuf::from("tools/windows/adb.exe")
    } else if cfg!(target_os = "macos") {
        PathBuf::from("tools/macos/adb")
    } else {
        PathBuf::from("tools/linux/adb")
    };

    AdbPaths { adb: project_root.join(relative) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_platform_adb_under_tools() {
        let paths = resolve_adb_path(Path::new("/app"));
        assert!(paths.adb.display().to_string().contains("tools"));
        assert!(paths.adb.display().to_string().contains("adb"));
    }
}
```

- [ ] **Step 3: Implement device context ingestion from any async line stream**

Create `engine/src/device.rs`:

```rust
use crate::filter::FilterQuery;
use crate::log_entry::{LogEntry, StatisticsSnapshot};
use crate::parser::parse_threadtime_line;
use crate::recorder::{Recorder, RecorderStatus};
use crate::ring_buffer::RingBuffer;
use crate::statistics::Statistics;

#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub logs: Vec<LogEntry>,
    pub stats: StatisticsSnapshot,
    pub recorder_status: RecorderStatus,
}

pub struct DeviceContext {
    pub device_id: String,
    pub device_name: String,
    seq: u64,
    filter: FilterQuery,
    buffer: RingBuffer<LogEntry>,
    statistics: Statistics,
    recorder: Recorder,
    recorder_status: RecorderStatus,
}

impl DeviceContext {
    pub fn new(device_id: String, device_name: String, buffer_capacity: usize, recorder: Recorder) -> Self {
        Self {
            device_id,
            device_name,
            seq: 0,
            filter: FilterQuery::default(),
            buffer: RingBuffer::new(buffer_capacity),
            statistics: Statistics::default(),
            recorder,
            recorder_status: RecorderStatus { enabled: false, path: None, warning: None },
        }
    }

    pub fn set_filter(&mut self, query: FilterQuery) {
        self.filter = query;
    }

    pub fn ingest_line(&mut self, raw_line: &str) -> Option<LogEntry> {
        self.seq += 1;
        let mut entry = parse_threadtime_line(self.seq, raw_line)?;
        entry.hidden = !self.filter.matches(&entry);
        self.statistics.observe(&entry);
        self.recorder_status = self.recorder.write_line(raw_line).unwrap_or_else(|error| RecorderStatus {
            enabled: false,
            path: None,
            warning: Some(error.to_string()),
        });
        self.buffer.push(entry.clone());
        Some(entry)
    }

    pub fn latest_visible_snapshot(&self, limit: usize) -> DeviceSnapshot {
        let logs = self
            .buffer
            .latest(limit)
            .into_iter()
            .filter(|entry| !entry.hidden)
            .collect();

        DeviceSnapshot {
            logs,
            stats: self.statistics.snapshot(),
            recorder_status: self.recorder_status.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::RecorderConfig;
    use tempfile::tempdir;

    #[test]
    fn ingests_and_snapshots_visible_lines() {
        let dir = tempdir().expect("tempdir");
        let recorder = Recorder::new(RecorderConfig {
            enabled: false,
            root: dir.path().to_path_buf(),
            device_name: "mock".to_string(),
        });
        let mut device = DeviceContext::new("mock".to_string(), "Mock".to_string(), 10, recorder);
        device.set_filter(FilterQuery::parse("level:error"));

        device.ingest_line("07-04 12:34:56.789  1234  5678 I Tag: info");
        device.ingest_line("07-04 12:34:57.789  1234  5678 E Tag: error");

        let snapshot = device.latest_visible_snapshot(500);
        assert_eq!(snapshot.logs.len(), 1);
        assert_eq!(snapshot.logs[0].message, "error");
        assert_eq!(snapshot.stats.errors, 1);
    }
}
```

- [ ] **Step 4: Export adb and device modules**

Replace `engine/src/main.rs` with:

```rust
mod adb;
mod device;
mod filter;
mod log_entry;
mod parser;
mod recorder;
mod ring_buffer;
mod statistics;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ALS_ENGINE_READY port=0");
    Ok(())
}
```

- [ ] **Step 5: Run device tests**

Run:

```bash
cargo test -p als-engine device adb -- --nocapture
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit device layer if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add engine/src/adb.rs engine/src/device.rs engine/src/main.rs tests/fixtures/sample-threadtime.log && git commit -m "feat: add device ingestion layer"
```

---

## Task 7: Implement WebSocket Server and Mock Data Mode

**Files:**
- Create: `engine/src/websocket.rs`
- Modify: `engine/src/main.rs`

- [ ] **Step 1: Implement typed WebSocket messages**

Create `engine/src/websocket.rs`:

```rust
use crate::device::DeviceContext;
use crate::filter::FilterQuery;
use crate::log_entry::{DeviceInfo, LogEntry, StatisticsSnapshot};
use crate::recorder::{Recorder, RecorderConfig};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio::time::{interval, Duration};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    ConnectDevice { device_id: String },
    DisconnectDevice { device_id: String },
    SetFilter { device_id: String, query: String },
    GetStatistics { device_id: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    DeviceList { devices: Vec<DeviceInfo> },
    NewLogs { device_id: String, logs: Vec<LogEntry> },
    Statistics { device_id: String, stats: StatisticsSnapshot },
    RecorderStatus { device_id: String, enabled: bool, path: Option<String>, warning: Option<String> },
    Error { message: String },
}

pub async fn run_server() -> anyhow::Result<u16> {
    let app = Router::new().route("/ws", get(ws_handler));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("websocket server failed: {error}");
        }
    });
    Ok(port)
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let recorder = Recorder::new(RecorderConfig {
        enabled: true,
        root: PathBuf::from("logs"),
        device_name: "mock-device".to_string(),
    });
    let mut device = DeviceContext::new("mock-device".to_string(), "Mock Device".to_string(), 1_000_000, recorder);

    let devices = ServerMessage::DeviceList {
        devices: vec![DeviceInfo { device_id: "mock-device".to_string(), device_name: "Mock Device".to_string(), connected: true }],
    };
    let _ = sender.send(Message::Text(serde_json::to_string(&devices).unwrap())).await;

    let mut ticker = interval(Duration::from_millis(250));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let raw = "07-04 12:34:56.789  1234  5678 I ActivityManager: Mock log line";
                if let Some(entry) = device.ingest_line(raw) {
                    if !entry.hidden {
                        let message = ServerMessage::NewLogs { device_id: "mock-device".to_string(), logs: vec![entry] };
                        if sender.send(Message::Text(serde_json::to_string(&message).unwrap())).await.is_err() {
                            break;
                        }
                    }
                }
                let snapshot = device.latest_visible_snapshot(500);
                let stats = ServerMessage::Statistics { device_id: "mock-device".to_string(), stats: snapshot.stats };
                let _ = sender.send(Message::Text(serde_json::to_string(&stats).unwrap())).await;
            }
            incoming = receiver.next() => {
                let Some(Ok(Message::Text(text))) = incoming else { break; };
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::SetFilter { query, .. }) => device.set_filter(FilterQuery::parse(&query)),
                    Ok(ClientMessage::GetStatistics { .. }) => {
                        let snapshot = device.latest_visible_snapshot(500);
                        let message = ServerMessage::Statistics { device_id: "mock-device".to_string(), stats: snapshot.stats };
                        let _ = sender.send(Message::Text(serde_json::to_string(&message).unwrap())).await;
                    }
                    Ok(ClientMessage::ConnectDevice { .. }) | Ok(ClientMessage::DisconnectDevice { .. }) => {}
                    Err(error) => {
                        let message = ServerMessage::Error { message: error.to_string() };
                        let _ = sender.send(Message::Text(serde_json::to_string(&message).unwrap())).await;
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Start the WebSocket server from engine main**

Replace `engine/src/main.rs` with:

```rust
mod adb;
mod device;
mod filter;
mod log_entry;
mod parser;
mod recorder;
mod ring_buffer;
mod statistics;
mod websocket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let port = websocket::run_server().await?;
    println!("ALS_ENGINE_READY port={port}");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

- [ ] **Step 3: Run backend tests**

Run:

```bash
cargo test -p als-engine
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Run engine and verify port output**

Run:

```bash
cargo run -p als-engine
```

Expected stdout:

```text
ALS_ENGINE_READY port=<dynamic-port>
```

Stop with `Ctrl+C`.

- [ ] **Step 5: Commit WebSocket server if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add engine/src/websocket.rs engine/src/main.rs && git commit -m "feat: expose engine websocket server"
```

---

## Task 8: Connect Electron Main Process to Rust Engine

**Files:**
- Modify: `src/main/main.ts`
- Modify: `src/main/preload.ts`
- Modify: `src/renderer/types/protocol.ts`

- [ ] **Step 1: Expose engine URL through preload**

Replace `src/main/preload.ts` with:

```ts
import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('als', {
  version: '0.1.0',
  getEngineUrl: () => ipcRenderer.invoke('engine:get-url') as Promise<string>,
});
```

- [ ] **Step 2: Add global renderer type**

Append to `src/renderer/types/protocol.ts`:

```ts
declare global {
  interface Window {
    als: {
      version: string;
      getEngineUrl: () => Promise<string>;
    };
  }
}
```

- [ ] **Step 3: Start the Rust engine from Electron**

Replace `src/main/main.ts` with:

```ts
import { app, BrowserWindow, ipcMain } from 'electron';
import path from 'node:path';
import { spawn, ChildProcessWithoutNullStreams } from 'node:child_process';

let engineProcess: ChildProcessWithoutNullStreams | null = null;
let engineUrl = 'ws://127.0.0.1:0/ws';

function engineBinaryPath() {
  const binary = process.platform === 'win32' ? 'als-engine.exe' : 'als-engine';
  return app.isPackaged
    ? path.join(process.resourcesPath, 'engine', binary)
    : path.join(process.cwd(), 'target', 'debug', binary);
}

function startEngine() {
  const binary = engineBinaryPath();
  engineProcess = spawn(binary, [], { cwd: process.cwd() });

  engineProcess.stdout.on('data', (chunk) => {
    const text = chunk.toString();
    const match = text.match(/ALS_ENGINE_READY port=(\d+)/);
    if (match) {
      engineUrl = `ws://127.0.0.1:${match[1]}/ws`;
    }
  });

  engineProcess.stderr.on('data', (chunk) => {
    console.error(`[als-engine] ${chunk.toString()}`);
  });
}

async function createWindow() {
  const win = new BrowserWindow({
    width: 1400,
    height: 900,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (process.env.VITE_DEV_SERVER_URL) {
    await win.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    await win.loadFile(path.join(__dirname, '../renderer/index.html'));
  }
}

ipcMain.handle('engine:get-url', () => engineUrl);

app.whenReady().then(() => {
  startEngine();
  createWindow();
});

app.on('before-quit', () => {
  engineProcess?.kill();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});
```

- [ ] **Step 4: Build Rust engine before launching Electron**

Run:

```bash
cargo build -p als-engine
npm run build
```

Expected:

```text
Rust debug binary exists at target/debug/als-engine or target/debug/als-engine.exe
Vite build succeeds
```

- [ ] **Step 5: Commit Electron engine bootstrap if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add src/main src/renderer/types/protocol.ts && git commit -m "feat: launch engine from electron"
```

---

## Task 9: Build Frontend WebSocket Client and State Store

**Files:**
- Create: `src/renderer/api/engineClient.ts`
- Create: `src/renderer/state/appStore.ts`
- Modify: `src/renderer/App.tsx`

- [ ] **Step 1: Create the Zustand store**

Create `src/renderer/state/appStore.ts`:

```ts
import { create } from 'zustand';
import type { DeviceInfo, LogEntry, ServerMessage, StatisticsSnapshot } from '../types/protocol';

const DEFAULT_VISIBLE_LIMIT = 500;
const MAX_VISIBLE_LIMIT = 5000;

interface AppStore {
  devices: DeviceInfo[];
  activeDeviceId: string | null;
  logs: LogEntry[];
  visibleLimit: number;
  filterQuery: string;
  searchQuery: string;
  stats: StatisticsSnapshot;
  connected: boolean;
  recorderPath: string | null;
  recorderWarning: string | null;
  setFilterQuery: (query: string) => void;
  setSearchQuery: (query: string) => void;
  handleServerMessage: (message: ServerMessage) => void;
}

const emptyStats: StatisticsSnapshot = {
  errors: 0,
  warnings: 0,
  logsPerSecond: 0,
  memoryBytes: 0,
  hidden: 0,
};

export const useAppStore = create<AppStore>((set, get) => ({
  devices: [],
  activeDeviceId: null,
  logs: [],
  visibleLimit: DEFAULT_VISIBLE_LIMIT,
  filterQuery: '',
  searchQuery: '',
  stats: emptyStats,
  connected: false,
  recorderPath: null,
  recorderWarning: null,
  setFilterQuery: (filterQuery) => set({ filterQuery }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  handleServerMessage: (message) => {
    if (message.type === 'device_list') {
      set({ devices: message.devices, activeDeviceId: message.devices[0]?.deviceId ?? null, connected: true });
      return;
    }

    if (message.type === 'new_logs') {
      const next = [...get().logs, ...message.logs].slice(-Math.min(get().visibleLimit, MAX_VISIBLE_LIMIT));
      set({ logs: next });
      return;
    }

    if (message.type === 'statistics') {
      set({ stats: message.stats });
      return;
    }

    if (message.type === 'recorder_status') {
      set({ recorderPath: message.path, recorderWarning: message.warning });
    }
  },
}));
```

- [ ] **Step 2: Create the WebSocket client**

Create `src/renderer/api/engineClient.ts`:

```ts
import type { ClientMessage, ServerMessage } from '../types/protocol';

export class EngineClient {
  private socket: WebSocket | null = null;

  constructor(private readonly onMessage: (message: ServerMessage) => void) {}

  async connect() {
    const url = await window.als.getEngineUrl();
    this.socket = new WebSocket(url);

    this.socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data) as ServerMessage;
      this.onMessage(message);
    });
  }

  send(message: ClientMessage) {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return;
    }
    this.socket.send(JSON.stringify(message));
  }
}
```

- [ ] **Step 3: Wire the client into App**

Replace `src/renderer/App.tsx` with:

```tsx
import { useEffect, useMemo } from 'react';
import { EngineClient } from './api/engineClient';
import { useAppStore } from './state/appStore';

export function App() {
  const handleServerMessage = useAppStore((state) => state.handleServerMessage);
  const connected = useAppStore((state) => state.connected);
  const logs = useAppStore((state) => state.logs);
  const client = useMemo(() => new EngineClient(handleServerMessage), [handleServerMessage]);

  useEffect(() => {
    client.connect();
  }, [client]);

  return (
    <main className="app-shell">
      <header className="toolbar">Android Logcat Studio</header>
      <section className="empty-state">
        {connected ? `Connected: ${logs.length} visible logs` : 'Connecting to engine...'}
      </section>
    </main>
  );
}
```

- [ ] **Step 4: Run frontend build**

Run:

```bash
npm run build
```

Expected:

```text
TypeScript compiles successfully
```

- [ ] **Step 5: Commit frontend client if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add src/renderer && git commit -m "feat: connect renderer to engine websocket"
```

---

## Task 10: Build MVP UI Components

**Files:**
- Create: `src/renderer/components/DeviceTabs.tsx`
- Create: `src/renderer/components/QueryBar.tsx`
- Create: `src/renderer/components/SearchBar.tsx`
- Create: `src/renderer/components/LogView.tsx`
- Create: `src/renderer/components/StatsPanel.tsx`
- Create: `src/renderer/components/StatusBar.tsx`
- Modify: `src/renderer/App.tsx`
- Modify: `src/renderer/styles.css`

- [ ] **Step 1: Create DeviceTabs**

Create `src/renderer/components/DeviceTabs.tsx`:

```tsx
import type { DeviceInfo } from '../types/protocol';

interface Props {
  devices: DeviceInfo[];
  activeDeviceId: string | null;
}

export function DeviceTabs({ devices, activeDeviceId }: Props) {
  return (
    <nav className="device-tabs" aria-label="Connected devices">
      {devices.map((device) => (
        <button key={device.deviceId} className={device.deviceId === activeDeviceId ? 'active' : ''}>
          {device.deviceName}
        </button>
      ))}
    </nav>
  );
}
```

- [ ] **Step 2: Create QueryBar**

Create `src/renderer/components/QueryBar.tsx`:

```tsx
interface Props {
  value: string;
  onChange: (value: string) => void;
}

export function QueryBar({ value, onChange }: Props) {
  return (
    <label className="query-bar">
      <span>Query Filter</span>
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder="package:launcher level:error tag:ActivityManager"
      />
    </label>
  );
}
```

- [ ] **Step 3: Create SearchBar**

Create `src/renderer/components/SearchBar.tsx`:

```tsx
interface Props {
  value: string;
  onChange: (value: string) => void;
}

export function SearchBar({ value, onChange }: Props) {
  return (
    <label className="search-bar">
      <span>Search</span>
      <input value={value} onChange={(event) => onChange(event.target.value)} placeholder="Ctrl+F" />
    </label>
  );
}
```

- [ ] **Step 4: Create LogView with virtual scrolling**

Create `src/renderer/components/LogView.tsx`:

```tsx
import { Virtuoso } from 'react-virtuoso';
import type { LogEntry } from '../types/protocol';

interface Props {
  logs: LogEntry[];
  searchQuery: string;
}

export function LogView({ logs, searchQuery }: Props) {
  return (
    <section className="log-view" aria-label="Log output">
      <Virtuoso
        data={logs}
        itemContent={(_, entry) => <LogRow entry={entry} searchQuery={searchQuery} />}
      />
    </section>
  );
}

function LogRow({ entry, searchQuery }: { entry: LogEntry; searchQuery: string }) {
  const message = highlight(entry.message, searchQuery);
  return (
    <div className={`log-row level-${entry.level}`}>
      <span className="log-time">{entry.time}</span>
      <span className="log-pid">{entry.pid}</span>
      <span className="log-tid">{entry.tid}</span>
      <span className="log-level">{entry.level[0].toUpperCase()}</span>
      <span className="log-tag">{entry.tag}</span>
      <span className="log-message">{message}</span>
    </div>
  );
}

function highlight(message: string, query: string) {
  if (!query) return message;
  const index = message.toLowerCase().indexOf(query.toLowerCase());
  if (index < 0) return message;

  return (
    <>
      {message.slice(0, index)}
      <mark>{message.slice(index, index + query.length)}</mark>
      {message.slice(index + query.length)}
    </>
  );
}
```

- [ ] **Step 5: Create StatsPanel**

Create `src/renderer/components/StatsPanel.tsx`:

```tsx
import type { StatisticsSnapshot } from '../types/protocol';

interface Props {
  stats: StatisticsSnapshot;
}

export function StatsPanel({ stats }: Props) {
  return (
    <aside className="stats-panel">
      <h2>Statistics</h2>
      <dl>
        <dt>Errors</dt><dd>{stats.errors}</dd>
        <dt>Warnings</dt><dd>{stats.warnings}</dd>
        <dt>Logs/s</dt><dd>{stats.logsPerSecond}</dd>
        <dt>Memory</dt><dd>{formatBytes(stats.memoryBytes)}</dd>
        <dt>Hidden</dt><dd>{stats.hidden}</dd>
      </dl>
    </aside>
  );
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${Math.round(value / 1024)} KB`;
  return `${Math.round(value / 1024 / 1024)} MB`;
}
```

- [ ] **Step 6: Create StatusBar**

Create `src/renderer/components/StatusBar.tsx`:

```tsx
interface Props {
  connected: boolean;
  recorderPath: string | null;
  recorderWarning: string | null;
  totalVisible: number;
}

export function StatusBar({ connected, recorderPath, recorderWarning, totalVisible }: Props) {
  return (
    <footer className="status-bar">
      <span>{connected ? 'Connected' : 'Disconnected'}</span>
      <span>{recorderPath ? `Recording: ${recorderPath}` : 'Recording pending'}</span>
      <span>{totalVisible} visible logs</span>
      {recorderWarning && <strong>{recorderWarning}</strong>}
    </footer>
  );
}
```

- [ ] **Step 7: Compose the UI in App**

Replace `src/renderer/App.tsx` with:

```tsx
import { DeviceTabs } from './components/DeviceTabs';
import { LogView } from './components/LogView';
import { QueryBar } from './components/QueryBar';
import { SearchBar } from './components/SearchBar';
import { StatsPanel } from './components/StatsPanel';
import { StatusBar } from './components/StatusBar';
import { useAppStore } from './state/appStore';

export function App() {
  const devices = useAppStore((state) => state.devices);
  const activeDeviceId = useAppStore((state) => state.activeDeviceId);
  const logs = useAppStore((state) => state.logs);
  const filterQuery = useAppStore((state) => state.filterQuery);
  const searchQuery = useAppStore((state) => state.searchQuery);
  const stats = useAppStore((state) => state.stats);
  const connected = useAppStore((state) => state.connected);
  const recorderPath = useAppStore((state) => state.recorderPath);
  const recorderWarning = useAppStore((state) => state.recorderWarning);
  const setFilterQuery = useAppStore((state) => state.setFilterQuery);
  const setSearchQuery = useAppStore((state) => state.setSearchQuery);

  return (
    <main className="app-shell">
      <header className="toolbar">
        <strong>Android Logcat Studio</strong>
        <SearchBar value={searchQuery} onChange={setSearchQuery} />
      </header>
      <DeviceTabs devices={devices} activeDeviceId={activeDeviceId} />
      <QueryBar value={filterQuery} onChange={setFilterQuery} />
      <section className="content-grid">
        <LogView logs={logs} searchQuery={searchQuery} />
        <StatsPanel stats={stats} />
      </section>
      <StatusBar connected={connected} recorderPath={recorderPath} recorderWarning={recorderWarning} totalVisible={logs.length} />
    </main>
  );
}
```

- [ ] **Step 8: Replace CSS with full layout styles**

Replace `src/renderer/styles.css`:

```css
:root {
  color-scheme: dark;
  font-family: Inter, Segoe UI, system-ui, sans-serif;
  background: #101318;
  color: #e6edf3;
}

body { margin: 0; }
button, input { font: inherit; }

.app-shell {
  min-height: 100vh;
  display: grid;
  grid-template-rows: 48px 40px 48px 1fr 32px;
  background: #101318;
}

.toolbar, .device-tabs, .query-bar, .status-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 16px;
  border-bottom: 1px solid #263140;
  background: #151a21;
}

.search-bar, .query-bar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.search-bar { margin-left: auto; }
.query-bar input, .search-bar input {
  min-width: 320px;
  color: #e6edf3;
  background: #0d1117;
  border: 1px solid #30363d;
  border-radius: 6px;
  padding: 6px 8px;
}

.device-tabs button {
  color: #c9d1d9;
  background: transparent;
  border: 0;
  padding: 8px 10px;
  border-radius: 6px;
}

.device-tabs button.active {
  background: #1f6feb;
  color: white;
}

.content-grid {
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
}

.log-view {
  min-width: 0;
  min-height: 0;
  font-family: Consolas, 'JetBrains Mono', monospace;
  font-size: 13px;
}

.log-row {
  display: grid;
  grid-template-columns: 110px 64px 64px 32px 180px minmax(0, 1fr);
  gap: 8px;
  padding: 2px 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  white-space: nowrap;
}

.log-message { overflow: hidden; text-overflow: ellipsis; }
.level-error, .level-assert { color: #ff6b6b; }
.level-warn { color: #ffd166; }
.level-info { color: #7ee787; }
.level-debug { color: #e6edf3; }
.level-verbose { color: #8b949e; }
mark { background: #f2cc60; color: #101318; }

.stats-panel {
  border-left: 1px solid #263140;
  padding: 16px;
  background: #151a21;
}

.stats-panel h2 { margin-top: 0; font-size: 14px; }
.stats-panel dl { display: grid; grid-template-columns: 1fr auto; gap: 8px; }
.stats-panel dt { color: #8b949e; }
.stats-panel dd { margin: 0; }
.status-bar { border-top: 1px solid #263140; border-bottom: 0; font-size: 12px; color: #8b949e; }
.status-bar strong { color: #ff6b6b; }
```

- [ ] **Step 9: Run frontend build**

Run:

```bash
npm run build
```

Expected:

```text
TypeScript and Vite build complete
```

- [ ] **Step 10: Commit UI components if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add src/renderer && git commit -m "feat: add MVP logcat interface"
```

---

## Task 11: Wire Query Filter Updates to Backend

**Files:**
- Modify: `src/renderer/state/appStore.ts`
- Modify: `src/renderer/api/engineClient.ts`
- Modify: `src/renderer/App.tsx`

- [ ] **Step 1: Let EngineClient report open state**

Replace `src/renderer/api/engineClient.ts` with:

```ts
import type { ClientMessage, ServerMessage } from '../types/protocol';

export class EngineClient {
  private socket: WebSocket | null = null;

  constructor(private readonly onMessage: (message: ServerMessage) => void) {}

  async connect() {
    const url = await window.als.getEngineUrl();
    this.socket = new WebSocket(url);

    this.socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data) as ServerMessage;
      this.onMessage(message);
    });
  }

  send(message: ClientMessage) {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return false;
    }
    this.socket.send(JSON.stringify(message));
    return true;
  }
}
```

- [ ] **Step 2: Send filter messages from App**

Replace `src/renderer/App.tsx` with:

```tsx
import { useEffect, useMemo } from 'react';
import { EngineClient } from './api/engineClient';
import { DeviceTabs } from './components/DeviceTabs';
import { LogView } from './components/LogView';
import { QueryBar } from './components/QueryBar';
import { SearchBar } from './components/SearchBar';
import { StatsPanel } from './components/StatsPanel';
import { StatusBar } from './components/StatusBar';
import { useAppStore } from './state/appStore';

export function App() {
  const devices = useAppStore((state) => state.devices);
  const activeDeviceId = useAppStore((state) => state.activeDeviceId);
  const logs = useAppStore((state) => state.logs);
  const filterQuery = useAppStore((state) => state.filterQuery);
  const searchQuery = useAppStore((state) => state.searchQuery);
  const stats = useAppStore((state) => state.stats);
  const connected = useAppStore((state) => state.connected);
  const recorderPath = useAppStore((state) => state.recorderPath);
  const recorderWarning = useAppStore((state) => state.recorderWarning);
  const setFilterQuery = useAppStore((state) => state.setFilterQuery);
  const setSearchQuery = useAppStore((state) => state.setSearchQuery);
  const handleServerMessage = useAppStore((state) => state.handleServerMessage);
  const client = useMemo(() => new EngineClient(handleServerMessage), [handleServerMessage]);

  useEffect(() => {
    client.connect();
  }, [client]);

  function handleFilterChange(next: string) {
    setFilterQuery(next);
    if (activeDeviceId) {
      client.send({ type: 'set_filter', deviceId: activeDeviceId, query: next });
    }
  }

  return (
    <main className="app-shell">
      <header className="toolbar">
        <strong>Android Logcat Studio</strong>
        <SearchBar value={searchQuery} onChange={setSearchQuery} />
      </header>
      <DeviceTabs devices={devices} activeDeviceId={activeDeviceId} />
      <QueryBar value={filterQuery} onChange={handleFilterChange} />
      <section className="content-grid">
        <LogView logs={logs} searchQuery={searchQuery} />
        <StatsPanel stats={stats} />
      </section>
      <StatusBar connected={connected} recorderPath={recorderPath} recorderWarning={recorderWarning} totalVisible={logs.length} />
    </main>
  );
}
```

- [ ] **Step 3: Run frontend build**

Run:

```bash
npm run build
```

Expected:

```text
TypeScript compiles successfully
```

- [ ] **Step 4: Commit filter wiring if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add src/renderer && git commit -m "feat: send filter updates to engine"
```

---

## Task 12: Add Backend Search Results for Full-Log Search

**Files:**
- Modify: `engine/src/websocket.rs`
- Modify: `engine/src/device.rs`
- Modify: `src/renderer/api/engineClient.ts`
- Modify: `src/renderer/App.tsx`

- [ ] **Step 1: Add search to DeviceContext**

In `engine/src/device.rs`, add this method inside `impl DeviceContext`:

```rust
    pub fn search_visible_sequences(&self, query: &str) -> Vec<u64> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle = query.to_ascii_lowercase();
        self.buffer
            .latest(1_000_000)
            .into_iter()
            .filter(|entry| entry.message.to_ascii_lowercase().contains(&needle))
            .map(|entry| entry.seq)
            .collect()
    }
```

- [ ] **Step 2: Extend websocket client message enum**

In `engine/src/websocket.rs`, replace `ClientMessage` with:

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    ConnectDevice { device_id: String },
    DisconnectDevice { device_id: String },
    SetFilter { device_id: String, query: String },
    SetSearch { device_id: String, query: String, options: serde_json::Value },
    GetStatistics { device_id: String },
}
```

- [ ] **Step 3: Handle search requests**

In `engine/src/websocket.rs`, add this match arm before `GetStatistics`:

```rust
                    Ok(ClientMessage::SetSearch { query, .. }) => {
                        let matches = device.search_visible_sequences(&query);
                        let message = ServerMessage::SearchResults {
                            device_id: "mock-device".to_string(),
                            matches,
                        };
                        let _ = sender.send(Message::Text(serde_json::to_string(&message).unwrap())).await;
                    }
```

- [ ] **Step 4: Add search result state to frontend store**

In `src/renderer/state/appStore.ts`, add `searchMatches: number[];` to `AppStore`, add `searchMatches: [],` to initial state, and add this block to `handleServerMessage`:

```ts
    if (message.type === 'search_results') {
      set({ searchMatches: message.matches });
      return;
    }
```

- [ ] **Step 5: Send full search requests from App**

In `src/renderer/App.tsx`, replace `SearchBar value={searchQuery} onChange={setSearchQuery}` with:

```tsx
<SearchBar
  value={searchQuery}
  onChange={(next) => {
    setSearchQuery(next);
    if (activeDeviceId) {
      client.send({
        type: 'set_search',
        deviceId: activeDeviceId,
        query: next,
        options: { regex: false, caseSensitive: false, wholeWord: false },
      });
    }
  }}
/>
```

- [ ] **Step 6: Run all checks**

Run:

```bash
cargo test -p als-engine
npm run build
```

Expected:

```text
Rust tests pass
TypeScript build passes
```

- [ ] **Step 7: Commit search if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add engine/src src/renderer && git commit -m "feat: add backend full-log search"
```

---

## Task 13: Add E2E Smoke Test with Mock Engine Logs

**Files:**
- Create: `playwright.config.ts`
- Create: `tests/e2e/app.spec.ts`
- Modify: `package.json`

- [ ] **Step 1: Add Playwright config**

Create `playwright.config.ts`:

```ts
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: 'tests/e2e',
  timeout: 30_000,
  use: {
    trace: 'on-first-retry',
  },
});
```

- [ ] **Step 2: Add browser-level smoke test for renderer UI**

Create `tests/e2e/app.spec.ts`:

```ts
import { test, expect } from '@playwright/test';

test('renders ALS shell and log panels', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'als', {
      value: {
        version: '0.1.0',
        getEngineUrl: async () => 'ws://127.0.0.1:65535/ws',
      },
    });
  });

  await page.goto('http://127.0.0.1:5173');

  await expect(page.getByText('Android Logcat Studio')).toBeVisible();
  await expect(page.getByLabel('Log output')).toBeVisible();
  await expect(page.getByText('Statistics')).toBeVisible();
});
```

- [ ] **Step 3: Ensure package scripts include E2E test**

Verify `package.json` has this script:

```json
"test:e2e": "playwright test"
```

- [ ] **Step 4: Run renderer server and E2E test**

In terminal A, run:

```bash
npm run dev
```

Expected:

```text
Local: http://127.0.0.1:5173/
```

In terminal B, run:

```bash
npm run test:e2e
```

Expected:

```text
1 passed
```

- [ ] **Step 5: Commit E2E smoke test if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add playwright.config.ts tests/e2e package.json && git commit -m "test: add renderer smoke test"
```

---

## Task 14: Add MVP Acceptance Checklist and Run Full Verification

**Files:**
- Create: `docs/superpowers/plans/2026-07-04-android-logcat-studio-mvp-acceptance.md`

- [ ] **Step 1: Create acceptance checklist document**

Create `docs/superpowers/plans/2026-07-04-android-logcat-studio-mvp-acceptance.md`:

```md
# Android Logcat Studio MVP Acceptance Checklist

- [ ] Windows app starts without Android Studio installed.
- [ ] Engine prints `ALS_ENGINE_READY port=<port>` and serves `/ws` on localhost.
- [ ] Renderer receives `device_list` and shows Mock Device in the device tab bar.
- [ ] Renderer displays incoming mock log lines.
- [ ] Visible log count remains at or below 500 by default.
- [ ] Query Filter sends `set_filter` to the backend.
- [ ] Current view search highlights matching text.
- [ ] Full search sends `set_search` to the backend and receives `search_results`.
- [ ] Recorder writes log files under `logs/<date>/<device>/`.
- [ ] Status bar shows connection state and recorder path.
- [ ] `cargo test -p als-engine` passes.
- [ ] `npm run build` passes.
- [ ] `npm run test:e2e` passes.
```

- [ ] **Step 2: Run backend verification**

Run:

```bash
cargo test -p als-engine
```

Expected:

```text
test result: ok
```

- [ ] **Step 3: Run frontend verification**

Run:

```bash
npm run build
```

Expected:

```text
TypeScript and Vite build complete without errors
```

- [ ] **Step 4: Run E2E verification**

Run the Vite dev server in terminal A:

```bash
npm run dev
```

Run Playwright in terminal B:

```bash
npm run test:e2e
```

Expected:

```text
1 passed
```

- [ ] **Step 5: Commit acceptance checklist if git is available**

Run:

```bash
git rev-parse --is-inside-work-tree && git add docs/superpowers/plans/2026-07-04-android-logcat-studio-mvp-acceptance.md && git commit -m "docs: add MVP acceptance checklist"
```

---

## Self-Review

Spec coverage:

- Product target and Windows-first MVP: covered by Tasks 1, 8, and 14.
- Electron + React/TypeScript frontend: covered by Tasks 1, 9, 10, and 11.
- Rust backend engine: covered by Tasks 1 through 7 and 12.
- Local WebSocket protocol: covered by Tasks 2, 7, 8, 9, and 12.
- LogEntry schema: covered by Task 2.
- Threadtime parsing: covered by Task 3.
- Ring buffer default design: covered by Tasks 4 and 6. The implementation path uses configurable capacity and passes `1_000_000` for the mock device.
- Recorder hourly write: covered by Task 5 and exposed through Tasks 7 and 10.
- Query Filter basic fields: covered by Task 4 and wired in Task 11.
- Current view search and backend search: covered by Tasks 10 and 12.
- Statistics panel: covered by Tasks 4, 7, 9, and 10.
- First-phase manual bookmark: not implemented in this MVP plan because it is not in the acceptance checklist for the first working vertical slice. Add a follow-up plan for manual bookmark persistence after the log pipeline is stable.
- Real adb integration: path resolution is covered by Task 6, while the MVP uses mock data mode. Add a follow-up plan to replace mock data with real `adb devices` and `adb logcat -v threadtime` process management once the WebSocket and UI loop are verified.

Placeholder scan:

- No `TBD` markers.
- No empty code blocks.
- No task says to add unspecified error handling.

Type consistency:

- Rust uses `serde(rename_all = "camelCase")`, matching TypeScript fields like `packageName` and `logsPerSecond`.
- WebSocket event names use `snake_case`, matching the approved protocol.
- Frontend store consumes `ServerMessage` variants defined in `src/renderer/types/protocol.ts`.
