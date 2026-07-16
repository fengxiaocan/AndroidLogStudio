import { create } from 'zustand';
import { DEFAULT_SELECTED_LEVELS } from '../components/QueryBar';
import { loadSettings, saveSettings } from '../settings/storage';
import {
  DEFAULT_SETTINGS,
  type AppSettings,
  type ColumnVisibility,
  type LevelColorKey,
  type LevelColors,
  type Locale,
  type LogColumnId,
} from '../settings/types';
import type { AdbStatus, DeviceInfo, LogEntry, LogLevel, ServerMessage, StatisticsSnapshot } from '../types/protocol';

export const DEFAULT_VISIBLE_LIMIT = DEFAULT_SETTINGS.maxVisibleRows;
export const MAX_VISIBLE_LIMIT = 5000;

export type FilterLevel = Exclude<LogLevel, 'unknown'>;

export const emptyStats: StatisticsSnapshot = {
  errors: 0,
  warnings: 0,
  logsPerSecond: 0,
  memoryBytes: 0,
  hidden: 0,
};

interface AppState {
  devices: DeviceInfo[];
  activeDeviceId: string | null;
  logs: LogEntry[];
  visibleLimit: number;
  packageFilter: string;
  tagFilter: string;
  selectedLevels: FilterLevel[];
  caseInsensitive: boolean;
  searchQuery: string;
  searchMatches: number[];
  stats: StatisticsSnapshot;
  connected: boolean;
  paused: boolean;
  adbStatus: AdbStatus | null;
  recorderPath: string | null;
  recorderWarning: string | null;
  settings: AppSettings;
  settingsOpen: boolean;
  setPackageFilter: (packageFilter: string) => void;
  setTagFilter: (tagFilter: string) => void;
  setSelectedLevels: (selectedLevels: FilterLevel[]) => void;
  setCaseInsensitive: (caseInsensitive: boolean) => void;
  toggleLevel: (level: FilterLevel) => void;
  setSearchQuery: (searchQuery: string) => void;
  setActiveDeviceId: (deviceId: string) => void;
  clearLogs: () => void;
  togglePaused: () => void;
  openSettings: () => void;
  closeSettings: () => void;
  setColumnVisible: (column: LogColumnId, visible: boolean) => void;
  setLevelColor: (level: LevelColorKey, color: string) => void;
  setMaxVisibleRows: (maxVisibleRows: number) => void;
  setLocale: (locale: Locale) => void;
  resetSettings: () => void;
  handleServerMessage: (message: ServerMessage) => void;
}

function persistSettings(settings: AppSettings) {
  saveSettings(settings);
  return settings;
}

function withSettings(partial: Partial<AppSettings>, current: AppSettings): AppSettings {
  return persistSettings({
    columns: partial.columns ? { ...current.columns, ...partial.columns } : current.columns,
    levelColors: partial.levelColors
      ? { ...current.levelColors, ...partial.levelColors }
      : current.levelColors,
    maxVisibleRows: partial.maxVisibleRows ?? current.maxVisibleRows,
    locale: partial.locale ?? current.locale,
  });
}

const initialSettings = loadSettings();

function nextActiveDeviceId(activeDeviceId: string | null, devices: DeviceInfo[]): string | null {
  if (activeDeviceId && devices.some((device) => device.deviceId === activeDeviceId)) {
    return activeDeviceId;
  }
  return devices[0]?.deviceId ?? null;
}

function emptyActiveDeviceState() {
  return {
    logs: [] as LogEntry[],
    searchMatches: [] as number[],
    stats: emptyStats,
    recorderPath: null as string | null,
    recorderWarning: null as string | null,
  };
}

export const useAppStore = create<AppState>((set) => ({
  devices: [],
  activeDeviceId: null,
  logs: [],
  visibleLimit: initialSettings.maxVisibleRows,
  packageFilter: '',
  tagFilter: '',
  selectedLevels: [...DEFAULT_SELECTED_LEVELS],
  caseInsensitive: false,
  searchQuery: '',
  searchMatches: [],
  stats: emptyStats,
  connected: false,
  paused: false,
  adbStatus: null,
  recorderPath: null,
  recorderWarning: null,
  settings: initialSettings,
  settingsOpen: false,
  setPackageFilter: (packageFilter) => set({ packageFilter }),
  setTagFilter: (tagFilter) => set({ tagFilter }),
  setSelectedLevels: (selectedLevels) => set({ selectedLevels }),
  setCaseInsensitive: (caseInsensitive: boolean) => set({ caseInsensitive }),
  toggleLevel: (level) =>
    set((state) => {
      const has = state.selectedLevels.includes(level);
      return {
        selectedLevels: has
          ? state.selectedLevels.filter((item) => item !== level)
          : [...state.selectedLevels, level],
      };
    }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setActiveDeviceId: (deviceId) =>
    set((state) => {
      if (deviceId === state.activeDeviceId) {
        return {};
      }
      if (!state.devices.some((device) => device.deviceId === deviceId)) {
        return {};
      }
      return {
        activeDeviceId: deviceId,
        ...emptyActiveDeviceState(),
      };
    }),
  clearLogs: () => set({ logs: [], searchMatches: [] }),
  togglePaused: () => set((state) => ({ paused: !state.paused })),
  openSettings: () => set({ settingsOpen: true }),
  closeSettings: () => set({ settingsOpen: false }),
  setColumnVisible: (column, visible) =>
    set((state) => {
      if (column === 'message') {
        return {};
      }
      const columns: ColumnVisibility = { ...state.settings.columns, [column]: visible };
      const settings = withSettings({ columns }, state.settings);
      return { settings };
    }),
  setLevelColor: (level, color) =>
    set((state) => {
      const levelColors: LevelColors = { ...state.settings.levelColors, [level]: color };
      return { settings: withSettings({ levelColors }, state.settings) };
    }),
  setMaxVisibleRows: (maxVisibleRows) =>
    set((state) => {
      const settings = withSettings({ maxVisibleRows }, state.settings);
      const limit = Math.min(settings.maxVisibleRows, MAX_VISIBLE_LIMIT);
      return {
        settings,
        visibleLimit: limit,
        logs: state.logs.slice(-limit),
      };
    }),
  setLocale: (locale) =>
    set((state) => ({
      settings: withSettings({ locale }, state.settings),
    })),
  resetSettings: () =>
    set((state) => {
      const settings = persistSettings({
        columns: { ...DEFAULT_SETTINGS.columns },
        levelColors: { ...DEFAULT_SETTINGS.levelColors },
        maxVisibleRows: DEFAULT_SETTINGS.maxVisibleRows,
        locale: DEFAULT_SETTINGS.locale,
      });
      const limit = Math.min(settings.maxVisibleRows, MAX_VISIBLE_LIMIT);
      return {
        settings,
        visibleLimit: limit,
        logs: state.logs.slice(-limit),
      };
    }),
  handleServerMessage: (message) => {
    switch (message.type) {
      case 'device_list':
        set((state) => {
          const activeDeviceId = nextActiveDeviceId(state.activeDeviceId, message.devices);
          return {
            devices: message.devices,
            activeDeviceId,
            connected: true,
            ...(activeDeviceId === state.activeDeviceId ? {} : emptyActiveDeviceState()),
          };
        });
        break;
      case 'new_logs':
        set((state) => {
          if (state.paused) {
            return {};
          }
          if (message.deviceId !== state.activeDeviceId) {
            return {};
          }
          const limit = Math.min(state.visibleLimit, MAX_VISIBLE_LIMIT);
          return { logs: [...state.logs, ...message.logs].slice(-limit) };
        });
        break;
      case 'log_snapshot':
        set((state) => {
          // Snapshots always apply (filter/device change); pause only freezes live stream.
          if (message.deviceId !== state.activeDeviceId) {
            return {};
          }
          const limit = Math.min(state.visibleLimit, MAX_VISIBLE_LIMIT);
          return { logs: message.logs.slice(-limit) };
        });
        break;
      case 'statistics':
        set((state) => (message.deviceId === state.activeDeviceId ? { stats: message.stats } : {}));
        break;
      case 'search_results':
        set((state) =>
          message.deviceId === state.activeDeviceId ? { searchMatches: message.matches } : {},
        );
        break;
      case 'recorder_status':
        set((state) =>
          message.deviceId === state.activeDeviceId
            ? { recorderPath: message.path, recorderWarning: message.warning }
            : {},
        );
        break;
      case 'adb_status':
        set({ adbStatus: message });
        break;
      case 'error':
        break;
    }
  },
}));
