import { beforeEach, describe, expect, it } from 'vitest';
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
  useAppStore.setState({
    devices: [],
    activeDeviceId: null,
    logs: [],
    visibleLimit: 500,
    filterQuery: '',
    searchQuery: '',
    searchMatches: [],
    stats: emptyStats,
    connected: false,
    recorderPath: null,
    recorderWarning: null,
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
});
