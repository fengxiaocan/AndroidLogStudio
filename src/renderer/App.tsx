import { useCallback, useEffect, useRef, useState } from 'react';
import { EngineClient } from './api/engineClient';
import { DeviceTabs } from './components/DeviceTabs';
import { LogView } from './components/LogView';
import { QueryBar } from './components/QueryBar';
import { SearchBar } from './components/SearchBar';
import { StatsPanel } from './components/StatsPanel';
import { StatusBar } from './components/StatusBar';
import { useAppStore } from './state/appStore';

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
  const [refreshWarning, setRefreshWarning] = useState<string | null>(null);
  const statusWarning = [recorderWarning, refreshWarning].filter(Boolean).join(' · ') || null;
  const activeDevice = devices.find((device) => device.deviceId === activeDeviceId);
  const canRemove = Boolean(activeDevice && !activeDevice.connected);

  useEffect(() => {
    if (hasConnectedRef.current) {
      return;
    }

    hasConnectedRef.current = true;
    clientRef.current = new EngineClient(handleServerMessage);
    void clientRef.current.connect();
  }, [handleServerMessage]);

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
