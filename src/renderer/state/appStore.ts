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
  stats: StatisticsSnapshot;
  connected: boolean;
  recorderPath: string | null;
  recorderWarning: string | null;
  setFilterQuery: (filterQuery: string) => void;
  setSearchQuery: (searchQuery: string) => void;
  handleServerMessage: (message: ServerMessage) => void;
}

export const useAppStore = create<AppState>((set) => ({
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
    switch (message.type) {
      case 'device_list':
        set({
          devices: message.devices,
          activeDeviceId: message.devices[0]?.deviceId ?? null,
          connected: true,
        });
        break;
      case 'new_logs':
        set((state) => {
          const limit = Math.min(state.visibleLimit, MAX_VISIBLE_LIMIT);
          return { logs: [...state.logs, ...message.logs].slice(-limit) };
        });
        break;
      case 'statistics':
        set({ stats: message.stats });
        break;
      case 'recorder_status':
        set({ recorderPath: message.path, recorderWarning: message.warning });
        break;
      case 'error':
        break;
    }
  },
}));
