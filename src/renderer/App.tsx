import { useCallback, useEffect, useRef, useState } from 'react';
import { EngineClient } from './api/engineClient';
import { DeviceSelect } from './components/DeviceSelect';
import { LogView } from './components/LogView';
import { composeFilterQuery, QueryBar } from './components/QueryBar';
import { SearchBar } from './components/SearchBar';
import { SettingsPanel } from './components/SettingsPanel';
import { StatsPanel } from './components/StatsPanel';
import { StatusBar } from './components/StatusBar';
import { buildExportFileName } from './export/fileName';
import { t } from './settings/i18n';
import { useAppStore, type FilterLevel } from './state/appStore';
import type { ExportMode, ServerMessage } from './types/protocol';

type ExportReadyMessage = Extract<ServerMessage, { type: 'export_ready' }>;

export function App() {
  const connected = useAppStore((state) => state.connected);
  const devices = useAppStore((state) => state.devices);
  const activeDeviceId = useAppStore((state) => state.activeDeviceId);
  const logs = useAppStore((state) => state.logs);
  const packageFilter = useAppStore((state) => state.packageFilter);
  const tagFilter = useAppStore((state) => state.tagFilter);
  const selectedLevels = useAppStore((state) => state.selectedLevels);
  const caseInsensitive = useAppStore((state) => state.caseInsensitive);
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
  const setCaseInsensitive = useAppStore((state) => state.setCaseInsensitive);
  const setSearchQuery = useAppStore((state) => state.setSearchQuery);
  const setActiveDeviceId = useAppStore((state) => state.setActiveDeviceId);
  const clearLogs = useAppStore((state) => state.clearLogs);
  const togglePaused = useAppStore((state) => state.togglePaused);
  const openSettings = useAppStore((state) => state.openSettings);
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
  const locale = settings.locale;
  const activeDevice = devices.find((device) => device.deviceId === activeDeviceId);
  const canRemove = Boolean(activeDevice && !activeDevice.connected);
  const canExport = Boolean(activeDeviceId && connected && !exportBusy);

  const sendFilter = useCallback(
    (pkg: string, tag: string, levels: ReadonlyArray<FilterLevel>, ci: boolean = caseInsensitive) => {
      if (!activeDeviceId) return;
      const query = composeFilterQuery(pkg, tag, levels, ci);
      clientRef.current?.send({ type: 'set_filter', deviceId: activeDeviceId, query });
    },
    [activeDeviceId, caseInsensitive],
  );

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

  // On active device change: request snapshot + re-apply filter (export/disconnect features).
  useEffect(() => {
    if (!activeDeviceId || !connected) return;
    clientRef.current?.send({ type: 'connect_device', deviceId: activeDeviceId });
    sendFilter(packageFilter, tagFilter, selectedLevels, caseInsensitive);
    // intentional: not package/tag/levels — those have their own handlers
  }, [activeDeviceId, connected, caseInsensitive]); // eslint-disable-line react-hooks/exhaustive-deps

  const handlePackageChange = useCallback(
    (next: string) => {
      setPackageFilter(next);
      sendFilter(next, tagFilter, selectedLevels, caseInsensitive);
    },
    [sendFilter, setPackageFilter, tagFilter, selectedLevels, caseInsensitive],
  );

  const handleTagChange = useCallback(
    (next: string) => {
      setTagFilter(next);
      sendFilter(packageFilter, next, selectedLevels, caseInsensitive);
    },
    [sendFilter, setTagFilter, packageFilter, selectedLevels, caseInsensitive],
  );

  const handleLevelToggle = useCallback(
    (level: FilterLevel) => {
      const has = selectedLevels.includes(level);
      const nextLevels = has
        ? selectedLevels.filter((item) => item !== level)
        : [...selectedLevels, level];
      toggleLevel(level);
      sendFilter(packageFilter, tagFilter, nextLevels, caseInsensitive);
    },
    [sendFilter, toggleLevel, packageFilter, tagFilter, selectedLevels, caseInsensitive],
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
          <button
            className="toolbar-btn"
            type="button"
            onClick={handleRemoveDevice}
            disabled={!canRemove}
            title={t(locale, 'removeDevice')}
          >
            {t(locale, 'removeDevice')}
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
          caseInsensitive={caseInsensitive}
          onPackageChange={handlePackageChange}
          onTagChange={handleTagChange}
          onLevelToggle={handleLevelToggle}
          onCaseInsensitiveChange={setCaseInsensitive}
          locale={locale}
        />
        <div className="query-region__actions">
          <button
            className="toolbar-btn"
            type="button"
            disabled={!canExport}
            onClick={() => void runExport('all')}
            title={t(locale, 'exportAll')}
          >
            {t(locale, 'exportAll')}
          </button>
          <button
            className="toolbar-btn"
            type="button"
            disabled={!canExport}
            onClick={() => void runExport('filtered')}
            title={t(locale, 'exportFiltered')}
          >
            {t(locale, 'exportFiltered')}
          </button>
        </div>
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
