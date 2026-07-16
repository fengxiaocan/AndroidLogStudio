import type { LogLevel } from '../types/protocol';

export type Locale = 'en' | 'zh';

export type LogColumnId = 'time' | 'pid' | 'tid' | 'level' | 'package' | 'tag' | 'message';

export type LevelColorKey = Exclude<LogLevel, 'unknown'>;

export interface ColumnVisibility {
  time: boolean;
  pid: boolean;
  tid: boolean;
  level: boolean;
  package: boolean;
  tag: boolean;
  message: boolean;
}

export interface LevelColors {
  verbose: string;
  debug: string;
  info: string;
  warn: string;
  error: string;
  assert: string;
}

export interface AppSettings {
  columns: ColumnVisibility;
  levelColors: LevelColors;
  maxVisibleRows: number;
  locale: Locale;
}

export const SETTINGS_STORAGE_KEY = 'als.settings.v1';

export const DEFAULT_LEVEL_COLORS: LevelColors = {
  verbose: '#c9d1d9',
  debug: '#58a6ff',
  info: '#7ee787',
  warn: '#ffd166',
  error: '#ff6b6b',
  assert: '#ff6b6b',
};

export const DEFAULT_COLUMNS: ColumnVisibility = {
  time: true,
  pid: true,
  tid: true,
  level: true,
  package: true,
  tag: true,
  message: true,
};

export const DEFAULT_SETTINGS: AppSettings = {
  columns: { ...DEFAULT_COLUMNS },
  levelColors: { ...DEFAULT_LEVEL_COLORS },
  maxVisibleRows: 500,
  locale: 'zh',
};

export const COLUMN_ORDER: LogColumnId[] = [
  'time',
  'pid',
  'tid',
  'level',
  'package',
  'tag',
  'message',
];

export const LEVEL_COLOR_ORDER: LevelColorKey[] = [
  'verbose',
  'debug',
  'info',
  'warn',
  'error',
  'assert',
];

export const MAX_VISIBLE_ROWS_OPTIONS = [100, 500, 1000, 2000, 5000] as const;
