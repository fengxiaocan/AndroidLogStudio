export type LogLevel = 'verbose' | 'debug' | 'info' | 'warn' | 'error' | 'assert' | 'unknown';

export type DeviceSource = 'adb' | 'mock';

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
  source: DeviceSource;
}

export interface AdbStatus {
  available: boolean;
  mode: 'bundled' | 'mock_fallback';
  path: string | null;
  message: string;
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
  | { type: 'remove_device'; deviceId: string }
  | { type: 'set_filter'; deviceId: string; query: string }
  | { type: 'set_search'; deviceId: string; query: string; options: SearchOptions }
  | { type: 'get_history'; deviceId: string; beforeSeq: number; limit: number }
  | { type: 'add_bookmark'; deviceId: string; seq: number }
  | { type: 'remove_bookmark'; deviceId: string; seq: number }
  | { type: 'get_statistics'; deviceId: string }
  | { type: 'refresh_devices' };

export interface SearchOptions {
  regex: boolean;
  caseSensitive: boolean;
  wholeWord: boolean;
}

export type ServerMessage =
  | { type: 'new_logs'; deviceId: string; logs: LogEntry[] }
  | { type: 'log_snapshot'; deviceId: string; logs: LogEntry[] }
  | { type: 'device_list'; devices: DeviceInfo[] }
  | { type: 'statistics'; deviceId: string; stats: StatisticsSnapshot }
  | { type: 'search_results'; deviceId: string; matches: number[] }
  | { type: 'recorder_status'; deviceId: string; enabled: boolean; path: string | null; warning: string | null }
  | { type: 'adb_status'; available: boolean; mode: 'bundled' | 'mock_fallback'; path: string | null; message: string }
  | { type: 'error'; message: string };

declare global {
  interface Window {
    als: {
      version: string;
      getEngineUrl: () => Promise<string>;
    };
  }
}
