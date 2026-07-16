import type { AdbStatus } from '../types/protocol';
import type { Locale } from '../settings/types';
import { t } from '../settings/i18n';

interface StatusBarProps {
  connected: boolean;
  adbStatus: AdbStatus | null;
  recorderPath: string | null;
  visibleLogCount: number;
  warning: string | null;
  paused?: boolean;
  locale: Locale;
}

export function StatusBar({
  connected,
  adbStatus,
  recorderPath,
  visibleLogCount,
  warning,
  paused = false,
  locale,
}: StatusBarProps) {
  const adbMessage = adbStatus?.message ?? `ADB: ${t(locale, 'pending')}`;

  return (
    <footer className="status-bar">
      <span className={connected ? 'status status--connected' : 'status status--disconnected'}>
        {connected ? t(locale, 'connected') : t(locale, 'disconnected')}
      </span>
      {paused ? (
        <span className="status status--paused" role="status">
          {t(locale, 'paused')}
        </span>
      ) : null}
      <span className="status-bar__adb" role="status" aria-live="polite" title={adbMessage}>
        {adbMessage}
      </span>
      <span>
        {t(locale, 'recorder')}: {recorderPath ?? t(locale, 'pending')}
      </span>
      <span>
        {visibleLogCount} {t(locale, 'visibleLogs')}
      </span>
      {warning ? <strong>{warning}</strong> : null}
    </footer>
  );
}
