import type { AdbStatus } from '../types/protocol';

interface StatusBarProps {
  connected: boolean;
  adbStatus: AdbStatus | null;
  recorderPath: string | null;
  visibleLogCount: number;
  warning: string | null;
}

export function StatusBar({ connected, adbStatus, recorderPath, visibleLogCount, warning }: StatusBarProps) {
  const adbMessage = adbStatus?.message ?? 'ADB: pending';

  return (
    <footer className="status-bar">
      <span className={connected ? 'status status--connected' : 'status status--disconnected'}>
        {connected ? 'connected' : 'disconnected'}
      </span>
      <span className="status-bar__adb" role="status" aria-live="polite" title={adbMessage}>
        {adbMessage}
      </span>
      <span>Recorder: {recorderPath ?? 'pending'}</span>
      <span>{visibleLogCount} visible logs</span>
      {warning ? <strong>{warning}</strong> : null}
    </footer>
  );
}
