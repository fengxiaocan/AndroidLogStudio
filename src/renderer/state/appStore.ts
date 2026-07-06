import { create } from 'zustand';
import type { DeviceInfo, LogEntry, ServerMessage, StatisticsSnapshot } from '../types/protocol';

export const DEFAULT_VISIBLE_LIMIT = 500;
export const MAX_VISIBLE_LIMIT = 5000;

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
  filterQuery: string;
  searchQuery: string;
  searchMatches: number[];
  stats: StatisticsSnapshot;
  connected: boolean;
  recorderPath: string | null;
  recorderWarning: string | null;
  setFilterQuery: (filterQuery: string) => void;
  setSearchQuery: (searchQuery: string) => void;
  handleServerMessage: (message: ServerMessage) => void;
}

function nextActiveDeviceId(activeDeviceId: string | null, devices: DeviceInfo[]): string | null {
  if (activeDeviceId && devices.some((device) => device.deviceId === activeDeviceId)) {
    return activeDeviceId;
  }
  return devices[0]?.deviceId ?? null;
}

function emptyActiveDeviceState() {
  return {
    logs: [],
    searchMatches: [],
    stats: emptyStats,
    recorderPath: null,
    recorderWarning: null,
  };
}

export const useAppStore = create<AppState>((set) => ({
  devices: [],
  activeDeviceId: null,
  logs: [],
  visibleLimit: DEFAULT_VISIBLE_LIMIT,
  filterQuery: '',
  searchQuery: '',
  searchMatches: [],
  stats: emptyStats,
  connected: false,
  recorderPath: null,
  recorderWarning: null,
  setFilterQuery: (filterQuery) => set({ filterQuery }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
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
          if (message.deviceId !== state.activeDeviceId) {
            return {};
          }
          const limit = Math.min(state.visibleLimit, MAX_VISIBLE_LIMIT);
          return { logs: [...state.logs, ...message.logs].slice(-limit) };
        });
        break;
      case 'log_snapshot':
        set((state) => {
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
      case 'error':
        break;
    }
  },
}));
