import { useCallback, useEffect, useRef, useState } from 'react';
import { EngineClient } from './api/engineClient';
import { DeviceSelect } from './components/DeviceSelect';
import { LogView } from './components/LogView';
import { composeFilterQuery, QueryBar } from './components/QueryBar';
import { SearchBar } from './components/SearchBar';
import { SettingsPanel } from './components/SettingsPanel';
import { StatsPanel } from './components/StatsPanel';
import { StatusBar } from './components/StatusBar';
import { t } from './settings/i18n';
import { useAppStore, type FilterLevel } from './state/appStore';

export function App() {
  const connected = useAppStore((state) => state.connected);
  const devices = useAppStore((state) => state.devices);
  const activeDeviceId = useAppStore((state) => state.activeDeviceId);
  const logs = useAppStore((state) => state.logs);
  const packageFilter = useAppStore((state) => state.packageFilter);
  const tagFilter = useAppStore((state) => state.tagFilter);
  const selectedLevels = useAppStore((state) => state.selectedLevels);
  const searchQuery = useAppStore((state) => state.searchQuery);
  const stats = useAppStore((state) => state.stats);
  const paused = useAppStore((state) => state.paused);
  const adbStatus = useAppStore((state) => state.adbStatus);
  const recorderPath = useAppStore((state) => state.recorderPath);
  const recorderWarning = useAppStore((state) => state.recorderWarning);
  const settings = useAppStore((state) => state.settings);
  const setPackageFilter = useAppStore((state) => state.setPackageFilter);
  const setTagFilter = useAppStore((state) => state.setTagFilter);
  const toggleLevel = useAppStore((state) => state.toggleLevel);
  const setSearchQuery = useAppStore((state) => state.setSearchQuery);
  const setActiveDeviceId = useAppStore((state) => state.setActiveDeviceId);
  const clearLogs = useAppStore((state) => state.clearLogs);
  const togglePaused = useAppStore((state) => state.togglePaused);
  const openSettings = useAppStore((state) => state.openSettings);
  const handleServerMessage = useAppStore((state) => state.handleServerMessage);
  const clientRef = useRef<EngineClient | null>(null);
  const hasConnectedRef = useRef(false);
  const [refreshWarning, setRefreshWarning] = useState<string | null>(null);
  const statusWarning = [recorderWarning, refreshWarning].filter(Boolean).join(' · ') || null;
  const locale = settings.locale;

  const sendFilter = useCallback(
    (pkg: string, tag: string, levels: ReadonlyArray<FilterLevel>) => {
      if (!activeDeviceId) return;
      const query = composeFilterQuery(pkg, tag, levels);
      clientRef.current?.send({ type: 'set_filter', deviceId: activeDeviceId, query });
    },
    [activeDeviceId],
  );

  useEffect(() => {
    if (hasConnectedRef.current) {
      return;
    }

    hasConnectedRef.current = true;
    clientRef.current = new EngineClient(handleServerMessage);
    void clientRef.current.connect();
  }, [handleServerMessage]);

  // Re-apply current filter when the active device changes (e.g. after refresh).
  useEffect(() => {
    if (!activeDeviceId || !connected) return;
    sendFilter(packageFilter, tagFilter, selectedLevels);
  }, [activeDeviceId, connected]); // eslint-disable-line react-hooks/exhaustive-deps

  const handlePackageChange = useCallback(
    (next: string) => {
      setPackageFilter(next);
      sendFilter(next, tagFilter, selectedLevels);
    },
    [sendFilter, setPackageFilter, tagFilter, selectedLevels],
  );

  const handleTagChange = useCallback(
    (next: string) => {
      setTagFilter(next);
      sendFilter(packageFilter, next, selectedLevels);
    },
    [sendFilter, setTagFilter, packageFilter, selectedLevels],
  );

  const handleLevelToggle = useCallback(
    (level: FilterLevel) => {
      const has = selectedLevels.includes(level);
      const nextLevels = has
        ? selectedLevels.filter((item) => item !== level)
        : [...selectedLevels, level];
      toggleLevel(level);
      sendFilter(packageFilter, tagFilter, nextLevels);
    },
    [sendFilter, toggleLevel, packageFilter, tagFilter, selectedLevels],
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

  const handleDeviceChange = useCallback(
    (deviceId: string) => {
      setActiveDeviceId(deviceId);
    },
    [setActiveDeviceId],
  );

  return (
    <main className="app-shell">
      <header className="toolbar">
        <div className="toolbar__left">
          <h1>{t(locale, 'appTitle')}</h1>
          <DeviceSelect
            devices={devices}
            activeDeviceId={activeDeviceId}
            onChange={handleDeviceChange}
            locale={locale}
          />
        </div>
        <div className="toolbar__right">
          <button className="toolbar-btn" type="button" onClick={clearLogs} title={t(locale, 'clear')}>
            {t(locale, 'clear')}
          </button>
          <button
            className={`toolbar-btn${paused ? ' toolbar-btn--active' : ''}`}
            type="button"
            onClick={togglePaused}
            title={paused ? t(locale, 'resume') : t(locale, 'pause')}
            aria-pressed={paused}
          >
            {paused ? t(locale, 'resume') : t(locale, 'pause')}
          </button>
          <button
            className="toolbar-btn"
            type="button"
            onClick={handleRefreshDevices}
            disabled={!connected}
            title={t(locale, 'refreshDevices')}
          >
            {t(locale, 'refreshDevices')}
          </button>
          <button className="toolbar-btn" type="button" onClick={openSettings} title={t(locale, 'settings')}>
            {t(locale, 'settings')}
          </button>
          <SearchBar value={searchQuery} onChange={handleSearchChange} locale={locale} />
        </div>
      </header>
      <section className="query-region" aria-label="Query controls">
        <QueryBar
          packageFilter={packageFilter}
          tagFilter={tagFilter}
          selectedLevels={selectedLevels}
          onPackageChange={handlePackageChange}
          onTagChange={handleTagChange}
          onLevelToggle={handleLevelToggle}
          locale={locale}
        />
      </section>
      <section className="content-grid" aria-label="Log workbench">
        <LogView logs={logs} searchQuery={searchQuery} settings={settings} />
        <StatsPanel stats={stats} locale={locale} />
      </section>
      <StatusBar
        connected={connected}
        adbStatus={adbStatus}
        recorderPath={recorderPath}
        visibleLogCount={logs.length}
        warning={statusWarning}
        paused={paused}
        locale={locale}
      />
      <SettingsPanel />
    </main>
  );
}
