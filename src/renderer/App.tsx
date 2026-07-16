import { useCallback, useEffect, useRef, useState } from 'react';
import { EngineClient } from './api/engineClient';
import { DeviceTabs } from './components/DeviceTabs';
import { LogView } from './components/LogView';
import { QueryBar } from './components/QueryBar';
import { SearchBar } from './components/SearchBar';
import { StatsPanel } from './components/StatsPanel';
import { StatusBar } from './components/StatusBar';
import { buildExportFileName } from './export/fileName';
import { useAppStore } from './state/appStore';
import type { ExportMode, ServerMessage } from './types/protocol';

type ExportReadyMessage = Extract<ServerMessage, { type: 'export_ready' }>;

export function App() {
  const connected = useAppStore((state) => state.connected);
  const devices = useAppStore((state) => state.devices);
  const activeDeviceId = useAppStore((state) => state.activeDeviceId);
  const logs = useAppStore((state) => state.logs);
  const filterQuery = useAppStore((state) => state.filterQuery);
  const searchQuery = useAppStore((state) => state.searchQuery);
  const stats = useAppStore((state) => state.stats);
  const adbStatus = useAppStore((state) => state.adbStatus);
  const recorderPath = useAppStore((state) => state.recorderPath);
  const recorderWarning = useAppStore((state) => state.recorderWarning);
  const setFilterQuery = useAppStore((state) => state.setFilterQuery);
  const setSearchQuery = useAppStore((state) => state.setSearchQuery);
  const setActiveDeviceId = useAppStore((state) => state.setActiveDeviceId);
  const handleServerMessage = useAppStore((state) => state.handleServerMessage);
  const clientRef = useRef<EngineClient | null>(null);
  const hasConnectedRef = useRef(false);
  const pendingExportRef = useRef<{
    deviceId: string;
    mode: ExportMode;
    resolve: (msg: ExportReadyMessage) => void;
    reject: (err: Error) => void;
  } | null>(null);
  const [refreshWarning, setRefreshWarning] = useState<string | null>(null);
  const [exportBusy, setExportBusy] = useState(false);
  const [exportHint, setExportHint] = useState<string | null>(null);
  const statusWarning =
    [recorderWarning, refreshWarning, exportHint].filter(Boolean).join(' · ') || null;
  const activeDevice = devices.find((device) => device.deviceId === activeDeviceId);
  const canRemove = Boolean(activeDevice && !activeDevice.connected);
  const canExport = Boolean(activeDeviceId && connected && !exportBusy);

  const onServerMessage = useCallback(
    (message: ServerMessage) => {
      if (message.type === 'export_ready') {
        const pending = pendingExportRef.current;
        if (
          pending &&
          pending.deviceId === message.deviceId &&
          pending.mode === message.mode
        ) {
          pendingExportRef.current = null;
          pending.resolve(message);
        }
      } else if (message.type === 'error' && pendingExportRef.current) {
        const pending = pendingExportRef.current;
        pendingExportRef.current = null;
        pending.reject(new Error(message.message));
      }
      handleServerMessage(message);
    },
    [handleServerMessage],
  );

  useEffect(() => {
    if (hasConnectedRef.current) {
      return;
    }

    hasConnectedRef.current = true;
    clientRef.current = new EngineClient(onServerMessage);
    void clientRef.current.connect();
  }, [onServerMessage]);

  // When active device changes (including engine-driven), request snapshot + re-apply filter
  useEffect(() => {
    if (!activeDeviceId || !connected) return;
    clientRef.current?.send({ type: 'connect_device', deviceId: activeDeviceId });
    if (filterQuery) {
      clientRef.current?.send({ type: 'set_filter', deviceId: activeDeviceId, query: filterQuery });
    }
    // intentional: not filterQuery — filter has its own handler
  }, [activeDeviceId, connected]);

  const handleDeviceChange = useCallback(
    (deviceId: string) => {
      setActiveDeviceId(deviceId);
      clientRef.current?.send({ type: 'connect_device', deviceId });
    },
    [setActiveDeviceId],
  );

  const handleRemoveDevice = useCallback(() => {
    if (!activeDeviceId) return;
    const device = devices.find((d) => d.deviceId === activeDeviceId);
    if (!device || device.connected) return;
    clientRef.current?.send({ type: 'remove_device', deviceId: activeDeviceId });
  }, [activeDeviceId, devices]);

  const handleFilterChange = useCallback(
    (next: string) => {
      setFilterQuery(next);

      if (activeDeviceId) {
        clientRef.current?.send({ type: 'set_filter', deviceId: activeDeviceId, query: next });
      }
    },
    [activeDeviceId, setFilterQuery],
  );

  const handleSearchChange = useCallback(
    (next: string) => {
      setSearchQuery(next);

      if (activeDeviceId) {
        clientRef.current?.send({
          type: 'set_search',
          deviceId: activeDeviceId,
          query: next,
          options: { regex: false, caseSensitive: false, wholeWord: false },
        });
      }
    },
    [activeDeviceId, setSearchQuery],
  );

  const handleRefreshDevices = useCallback(() => {
    const sent = clientRef.current?.send({ type: 'refresh_devices' }) ?? false;
    setRefreshWarning(sent ? null : 'Unable to refresh devices while disconnected');
  }, []);

  const waitForExportReady = useCallback((deviceId: string, mode: ExportMode, timeoutMs = 60_000) => {
    return new Promise<ExportReadyMessage>((resolve, reject) => {
      const timer = setTimeout(() => {
        if (pendingExportRef.current) {
          pendingExportRef.current = null;
          reject(new Error('export timed out'));
        }
      }, timeoutMs);
      pendingExportRef.current = {
        deviceId,
        mode,
        resolve: (msg) => {
          clearTimeout(timer);
          resolve(msg);
        },
        reject: (err) => {
          clearTimeout(timer);
          reject(err);
        },
      };
    });
  }, []);

  const runExport = useCallback(
    async (mode: ExportMode) => {
      if (!activeDeviceId || exportBusy) return;
      setExportBusy(true);
      setExportHint(null);
      try {
        const wait = waitForExportReady(activeDeviceId, mode);
        const sent = clientRef.current?.send({
          type: 'export_logs',
          deviceId: activeDeviceId,
          mode,
        });
        if (!sent) {
          pendingExportRef.current = null;
          throw new Error('Unable to export while disconnected');
        }
        const ready = await wait;
        const device = devices.find((d) => d.deviceId === activeDeviceId);
        const defaultName = buildExportFileName(device?.deviceName ?? activeDeviceId, mode);
        const saved = await window.als.exportSave(ready.path, defaultName);
        if (saved.canceled) {
          setExportHint('Export canceled');
        } else if (saved.error) {
          setExportHint(`Export failed: ${saved.error}`);
        } else {
          setExportHint(`Exported ${ready.lineCount} lines`);
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        setExportHint(message);
      } finally {
        setExportBusy(false);
      }
    },
    [activeDeviceId, devices, exportBusy, waitForExportReady],
  );

  return (
    <main className="app-shell">
      <header className="toolbar">
        <h1>Android Logcat Studio</h1>
        <SearchBar value={searchQuery} onChange={handleSearchChange} />
      </header>
      <DeviceTabs
        devices={devices}
        activeDeviceId={activeDeviceId}
        onSelect={handleDeviceChange}
      />
      <section className="query-region" aria-label="Query controls">
        <QueryBar value={filterQuery} onChange={handleFilterChange} />
        <div className="query-region__actions">
          <button
            className="refresh-devices"
            type="button"
            onClick={handleRefreshDevices}
            disabled={!connected}
          >
            Refresh Devices
          </button>
          <button
            className="refresh-devices"
            type="button"
            onClick={handleRemoveDevice}
            disabled={!canRemove}
            title="Remove device"
          >
            Remove device
          </button>
          <button
            className="refresh-devices"
            type="button"
            disabled={!canExport}
            onClick={() => void runExport('all')}
          >
            Export all
          </button>
          <button
            className="refresh-devices"
            type="button"
            disabled={!canExport}
            onClick={() => void runExport('filtered')}
          >
            Export filtered
          </button>
        </div>
      </section>
      <section className="content-grid" aria-label="Log workbench">
        <LogView logs={logs} searchQuery={searchQuery} />
        <StatsPanel stats={stats} />
      </section>
      <StatusBar
        connected={connected}
        adbStatus={adbStatus}
        recorderPath={recorderPath}
        visibleLogCount={logs.length}
        warning={statusWarning}
      />
    </main>
  );
}
