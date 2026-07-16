import { beforeEach, describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS } from '../settings/types';
import { emptyStats, useAppStore } from './appStore';
import type { DeviceInfo, LogEntry, StatisticsSnapshot } from '../types/protocol';

const deviceA: DeviceInfo = {
  deviceId: 'device-a',
  deviceName: 'Device A',
  connected: true,
  source: 'adb',
};

const deviceB: DeviceInfo = {
  deviceId: 'device-b',
  deviceName: 'Device B',
  connected: true,
  source: 'adb',
};

function logEntry(seq: number): LogEntry {
  return {
    seq,
    timestamp: 0,
    date: '07-04',
    time: '12:00:00.000',
    pid: 1234,
    tid: 5678,
    level: 'info',
    tag: 'ActivityManager',
    message: `log ${seq}`,
    packageName: 'com.example',
    foreground: null,
    background: null,
    hidden: false,
    bookmarked: false,
  };
}

const stats: StatisticsSnapshot = {
  errors: 1,
  warnings: 2,
  logsPerSecond: 3,
  memoryBytes: 4,
  hidden: 5,
};

beforeEach(() => {
  localStorage.clear();
  useAppStore.setState({
    devices: [],
    activeDeviceId: null,
    logs: [],
    visibleLimit: 500,
    packageFilter: '',
    tagFilter: '',
    selectedLevels: ['verbose', 'debug', 'info', 'warn', 'error', 'assert'],
    searchQuery: '',
    searchMatches: [],
    stats: emptyStats,
    connected: false,
    paused: false,
    adbStatus: null,
    recorderPath: null,
    recorderWarning: null,
    settings: {
      columns: { ...DEFAULT_SETTINGS.columns },
      levelColors: { ...DEFAULT_SETTINGS.levelColors },
      maxVisibleRows: DEFAULT_SETTINGS.maxVisibleRows,
      locale: DEFAULT_SETTINGS.locale,
    },
    settingsOpen: false,
  });
});

describe('appStore per-device server messages', () => {
  it('ignores log and status messages for inactive devices', () => {
    const { handleServerMessage } = useAppStore.getState();
    handleServerMessage({ type: 'device_list', devices: [deviceA, deviceB] });

    handleServerMessage({ type: 'new_logs', deviceId: deviceB.deviceId, logs: [logEntry(1)] });
    handleServerMessage({ type: 'statistics', deviceId: deviceB.deviceId, stats });
    handleServerMessage({
      type: 'recorder_status',
      deviceId: deviceB.deviceId,
      enabled: true,
      path: 'logs/device-b.log',
      warning: 'disk nearly full',
    });
    handleServerMessage({ type: 'search_results', deviceId: deviceB.deviceId, matches: [1] });

    expect(useAppStore.getState().logs).toEqual([]);
    expect(useAppStore.getState().stats).toEqual(emptyStats);
    expect(useAppStore.getState().recorderPath).toBeNull();
    expect(useAppStore.getState().recorderWarning).toBeNull();
    expect(useAppStore.getState().searchMatches).toEqual([]);
  });

  it('clears active device state when device list selects a different active device', () => {
    const { handleServerMessage } = useAppStore.getState();
    handleServerMessage({ type: 'device_list', devices: [deviceA] });
    handleServerMessage({ type: 'new_logs', deviceId: deviceA.deviceId, logs: [logEntry(1)] });
    handleServerMessage({ type: 'statistics', deviceId: deviceA.deviceId, stats });
    handleServerMessage({
      type: 'recorder_status',
      deviceId: deviceA.deviceId,
      enabled: true,
      path: 'logs/device-a.log',
      warning: 'disk nearly full',
    });
    handleServerMessage({ type: 'search_results', deviceId: deviceA.deviceId, matches: [1] });

    handleServerMessage({ type: 'device_list', devices: [deviceB] });

    expect(useAppStore.getState().activeDeviceId).toBe(deviceB.deviceId);
    expect(useAppStore.getState().logs).toEqual([]);
    expect(useAppStore.getState().stats).toEqual(emptyStats);
    expect(useAppStore.getState().recorderPath).toBeNull();
    expect(useAppStore.getState().recorderWarning).toBeNull();
    expect(useAppStore.getState().searchMatches).toEqual([]);
  });

  it('clearLogs empties the visible log list', () => {
    const { handleServerMessage, clearLogs } = useAppStore.getState();
    handleServerMessage({ type: 'device_list', devices: [deviceA] });
    handleServerMessage({ type: 'new_logs', deviceId: deviceA.deviceId, logs: [logEntry(1), logEntry(2)] });
    expect(useAppStore.getState().logs).toHaveLength(2);

    clearLogs();
    expect(useAppStore.getState().logs).toEqual([]);
    expect(useAppStore.getState().searchMatches).toEqual([]);
  });

  it('paused freezes new_logs but still applies log_snapshot', () => {
    const { handleServerMessage, togglePaused } = useAppStore.getState();
    handleServerMessage({ type: 'device_list', devices: [deviceA] });
    handleServerMessage({ type: 'new_logs', deviceId: deviceA.deviceId, logs: [logEntry(1)] });
    togglePaused();
    expect(useAppStore.getState().paused).toBe(true);

    handleServerMessage({ type: 'new_logs', deviceId: deviceA.deviceId, logs: [logEntry(2)] });
    expect(useAppStore.getState().logs.map((log) => log.seq)).toEqual([1]);

    handleServerMessage({ type: 'log_snapshot', deviceId: deviceA.deviceId, logs: [logEntry(9)] });
    expect(useAppStore.getState().logs.map((log) => log.seq)).toEqual([9]);
  });

  it('setActiveDeviceId switches device and clears logs', () => {
    const { handleServerMessage, setActiveDeviceId } = useAppStore.getState();
    handleServerMessage({ type: 'device_list', devices: [deviceA, deviceB] });
    handleServerMessage({ type: 'new_logs', deviceId: deviceA.deviceId, logs: [logEntry(1)] });

    setActiveDeviceId(deviceB.deviceId);
    expect(useAppStore.getState().activeDeviceId).toBe(deviceB.deviceId);
    expect(useAppStore.getState().logs).toEqual([]);
  });

  it('device_list connected flip keeps logs for same active device', () => {
    const { handleServerMessage } = useAppStore.getState();
    handleServerMessage({ type: 'device_list', devices: [deviceA, deviceB] });
    handleServerMessage({
      type: 'new_logs',
      deviceId: deviceA.deviceId,
      logs: [logEntry(1)],
    });

    handleServerMessage({
      type: 'device_list',
      devices: [{ ...deviceA, connected: false }, deviceB],
    });

    expect(useAppStore.getState().activeDeviceId).toBe(deviceA.deviceId);
    expect(useAppStore.getState().logs.map((l) => l.seq)).toEqual([1]);
    expect(
      useAppStore.getState().devices.find((d) => d.deviceId === deviceA.deviceId)?.connected,
    ).toBe(false);
  });

  it('device_list removing active device switches and clears logs', () => {
    const { handleServerMessage } = useAppStore.getState();
    handleServerMessage({ type: 'device_list', devices: [deviceA, deviceB] });
    handleServerMessage({
      type: 'new_logs',
      deviceId: deviceA.deviceId,
      logs: [logEntry(1)],
    });

    handleServerMessage({ type: 'device_list', devices: [deviceB] });

    expect(useAppStore.getState().activeDeviceId).toBe(deviceB.deviceId);
    expect(useAppStore.getState().logs).toEqual([]);
  });

  it('settings update columns colors rows and locale with persistence', () => {
    const { setColumnVisible, setLevelColor, setMaxVisibleRows, setLocale, resetSettings } =
      useAppStore.getState();

    setColumnVisible('pid', false);
    setLevelColor('error', '#abcdef');
    setMaxVisibleRows(1000);
    setLocale('en');

    const state = useAppStore.getState();
    expect(state.settings.columns.pid).toBe(false);
    expect(state.settings.levelColors.error).toBe('#abcdef');
    expect(state.settings.maxVisibleRows).toBe(1000);
    expect(state.visibleLimit).toBe(1000);
    expect(state.settings.locale).toBe('en');
    expect(localStorage.getItem('als.settings.v1')).toContain('"locale":"en"');

    resetSettings();
    expect(useAppStore.getState().settings).toEqual(DEFAULT_SETTINGS);
    expect(useAppStore.getState().visibleLimit).toBe(DEFAULT_SETTINGS.maxVisibleRows);
  });

  it('setMaxVisibleRows trims the visible log list', () => {
    const { handleServerMessage, setMaxVisibleRows } = useAppStore.getState();
    handleServerMessage({ type: 'device_list', devices: [deviceA] });
    handleServerMessage({
      type: 'new_logs',
      deviceId: deviceA.deviceId,
      logs: [logEntry(1), logEntry(2), logEntry(3)],
    });

    setMaxVisibleRows(100);
    // Still under the new limit, so all logs remain.
    expect(useAppStore.getState().logs).toHaveLength(3);

    useAppStore.setState({
      logs: Array.from({ length: 150 }, (_, index) => logEntry(index + 1)),
      visibleLimit: 500,
    });
    setMaxVisibleRows(100);
    expect(useAppStore.getState().logs).toHaveLength(100);
    expect(useAppStore.getState().logs[0]?.seq).toBe(51);
  });
});
