interface StatusBarProps {
  connected: boolean;
  recorderPath: string | null;
  visibleLogCount: number;
  warning: string | null;
}

export function StatusBar({ connected, recorderPath, visibleLogCount, warning }: StatusBarProps) {
  return (
    <footer className="status-bar">
      <span className={connected ? 'status status--connected' : 'status status--disconnected'}>
        {connected ? 'connected' : 'disconnected'}
      </span>
      <span>Recorder: {recorderPath ?? 'pending'}</span>
      <span>{visibleLogCount} visible logs</span>
      {warning ? <strong>{warning}</strong> : null}
    </footer>
  );
}
