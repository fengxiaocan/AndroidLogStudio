import type { Locale, LogColumnId, LevelColorKey } from './types';

type MessageKey =
  | 'appTitle'
  | 'settings'
  | 'settingsTitle'
  | 'close'
  | 'resetDefaults'
  | 'columnsSection'
  | 'colorsSection'
  | 'displaySection'
  | 'languageSection'
  | 'maxVisibleRows'
  | 'language'
  | 'clear'
  | 'pause'
  | 'resume'
  | 'refreshDevices'
  | 'removeDevice'
  | 'exportAll'
  | 'exportFiltered'
  | 'deviceDisconnected'
  | 'search'
  | 'package'
  | 'tag'
  | 'tagPlaceholder'
  | 'caseInsensitive'
  | 'level'
  | 'connected'
  | 'disconnected'
  | 'paused'
  | 'visibleLogs'
  | 'recorder'
  | 'pending'
  | 'noDevices'
  | 'device'
  | 'column.time'
  | 'column.pid'
  | 'column.tid'
  | 'column.level'
  | 'column.package'
  | 'column.tag'
  | 'column.message'
  | 'level.verbose'
  | 'level.debug'
  | 'level.info'
  | 'level.warn'
  | 'level.error'
  | 'level.assert'
  | 'locale.en'
  | 'locale.zh'
  | 'stats.errors'
  | 'stats.warnings'
  | 'stats.rate'
  | 'stats.memory'
  | 'stats.hidden';

const en: Record<MessageKey, string> = {
  appTitle: 'Android Logcat Studio',
  settings: 'Settings',
  settingsTitle: 'Settings',
  close: 'Close',
  resetDefaults: 'Reset defaults',
  columnsSection: 'Visible columns',
  colorsSection: 'Level colors',
  displaySection: 'Display',
  languageSection: 'Language',
  maxVisibleRows: 'Max visible rows',
  language: 'Language',
  clear: 'Clear',
  pause: 'Pause',
  resume: 'Resume',
  refreshDevices: 'Refresh Devices',
  removeDevice: 'Remove device',
  exportAll: 'Export all',
  exportFiltered: 'Export filtered',
  deviceDisconnected: 'Disconnected',
  search: 'Search',
  package: 'Package',
  tag: 'Tag',
  tagPlaceholder: 'ActivityManager | Choreographer',
  caseInsensitive: 'Ignore case',
  level: 'Level',
  connected: 'Connected',
  disconnected: 'Disconnected',
  paused: 'Paused',
  visibleLogs: 'visible logs',
  recorder: 'Recorder',
  pending: 'pending',
  noDevices: 'No devices connected',
  device: 'Device',
  'column.time': 'Time',
  'column.pid': 'PID',
  'column.tid': 'TID',
  'column.level': 'Level',
  'column.package': 'Package',
  'column.tag': 'Tag',
  'column.message': 'Message',
  'level.verbose': 'Verbose',
  'level.debug': 'Debug',
  'level.info': 'Info',
  'level.warn': 'Warn',
  'level.error': 'Error',
  'level.assert': 'Assert',
  'locale.en': 'English',
  'locale.zh': '中文',
  'stats.errors': 'Errors',
  'stats.warnings': 'Warnings',
  'stats.rate': 'Logs / s',
  'stats.memory': 'Memory',
  'stats.hidden': 'Hidden',
};

const zh: Record<MessageKey, string> = {
  appTitle: 'Android Logcat Studio',
  settings: '设置',
  settingsTitle: '设置',
  close: '关闭',
  resetDefaults: '恢复默认',
  columnsSection: '显示列',
  colorsSection: '日志等级颜色',
  displaySection: '显示',
  languageSection: '语言',
  maxVisibleRows: '最大显示行数',
  language: '语言',
  clear: '清除',
  pause: '暂停',
  resume: '继续',
  refreshDevices: '刷新设备',
  removeDevice: '移除设备',
  exportAll: '导出全部',
  exportFiltered: '导出过滤',
  deviceDisconnected: '已断开',
  search: '搜索',
  package: '包名',
  tag: '标签',
  tagPlaceholder: 'ActivityManager | Choreographer | WindowManager',
  caseInsensitive: '忽略大小写',
  level: '等级',
  connected: '已连接',
  disconnected: '未连接',
  paused: '已暂停',
  visibleLogs: '条可见日志',
  recorder: '录制',
  pending: '等待中',
  noDevices: '未连接设备',
  device: '设备',
  'column.time': '时间',
  'column.pid': 'PID',
  'column.tid': 'TID',
  'column.level': '等级',
  'column.package': '包名',
  'column.tag': '标签',
  'column.message': '消息',
  'level.verbose': 'Verbose',
  'level.debug': 'Debug',
  'level.info': 'Info',
  'level.warn': 'Warn',
  'level.error': 'Error',
  'level.assert': 'Assert',
  'locale.en': 'English',
  'locale.zh': '中文',
  'stats.errors': '错误',
  'stats.warnings': '警告',
  'stats.rate': '日志 / 秒',
  'stats.memory': '内存',
  'stats.hidden': '已隐藏',
};

const catalogs: Record<Locale, Record<MessageKey, string>> = { en, zh };

export type { MessageKey };

export function t(locale: Locale, key: MessageKey): string {
  return catalogs[locale][key] ?? catalogs.en[key] ?? key;
}

export function columnLabel(locale: Locale, column: LogColumnId): string {
  return t(locale, `column.${column}` as MessageKey);
}

export function levelLabel(locale: Locale, level: LevelColorKey): string {
  return t(locale, `level.${level}` as MessageKey);
}
