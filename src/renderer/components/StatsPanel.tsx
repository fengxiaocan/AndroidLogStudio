import type { StatisticsSnapshot } from '../types/protocol';
import type { Locale } from '../settings/types';
import { t } from '../settings/i18n';

interface StatsPanelProps {
  stats: StatisticsSnapshot;
  locale: Locale;
}

function formatMemory(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const kilobytes = bytes / 1024;
  if (kilobytes < 1024) {
    return `${kilobytes.toFixed(1)} KB`;
  }

  return `${(kilobytes / 1024).toFixed(1)} MB`;
}

export function StatsPanel({ stats, locale }: StatsPanelProps) {
  return (
    <aside className="stats-panel" aria-label="Log statistics">
      <dl className="stats-list">
        <div className="stats-item stats-item--error">
          <dt>{t(locale, 'stats.errors')}</dt>
          <dd>{stats.errors}</dd>
        </div>
        <div className="stats-item stats-item--warn">
          <dt>{t(locale, 'stats.warnings')}</dt>
          <dd>{stats.warnings}</dd>
        </div>
        <div className="stats-item">
          <dt>{t(locale, 'stats.rate')}</dt>
          <dd>{stats.logsPerSecond.toFixed(1)}</dd>
        </div>
        <div className="stats-item">
          <dt>{t(locale, 'stats.memory')}</dt>
          <dd>{formatMemory(stats.memoryBytes)}</dd>
        </div>
        <div className="stats-item">
          <dt>{t(locale, 'stats.hidden')}</dt>
          <dd>{stats.hidden}</dd>
        </div>
      </dl>
    </aside>
  );
}
