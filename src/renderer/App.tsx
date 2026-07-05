import { useCallback, useEffect, useRef } from 'react';
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
  const recorderPath = useAppStore((state) => state.recorderPath);
  const recorderWarning = useAppStore((state) => state.recorderWarning);
  const setFilterQuery = useAppStore((state) => state.setFilterQuery);
  const setSearchQuery = useAppStore((state) => state.setSearchQuery);
  const handleServerMessage = useAppStore((state) => state.handleServerMessage);
  const clientRef = useRef<EngineClient | null>(null);
  const hasConnectedRef = useRef(false);

  useEffect(() => {
    if (hasConnectedRef.current) {
      return;
    }

    hasConnectedRef.current = true;
    clientRef.current = new EngineClient(handleServerMessage);
    void clientRef.current.connect();
  }, [handleServerMessage]);

  const handleFilterChange = useCallback(
    (next: string) => {
      setFilterQuery(next);

      if (activeDeviceId) {
        clientRef.current?.send({ type: 'set_filter', deviceId: activeDeviceId, query: next });
      }
    },
    [activeDeviceId, setFilterQuery],
  );

  return (
    <main className="app-shell">
      <header className="toolbar">
        <h1>Android Logcat Studio</h1>
        <SearchBar value={searchQuery} onChange={setSearchQuery} />
      </header>
      <DeviceTabs devices={devices} activeDeviceId={activeDeviceId} />
      <section className="query-region" aria-label="Query controls">
        <QueryBar value={filterQuery} onChange={handleFilterChange} />
      </section>
      <section className="content-grid" aria-label="Log workbench">
        <LogView logs={logs} searchQuery={searchQuery} />
        <StatsPanel stats={stats} />
      </section>
      <StatusBar
        connected={connected}
        recorderPath={recorderPath}
        visibleLogCount={logs.length}
        warning={recorderWarning}
      />
    </main>
  );
}
