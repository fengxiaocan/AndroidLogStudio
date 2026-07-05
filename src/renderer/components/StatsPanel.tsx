import type { StatisticsSnapshot } from '../types/protocol';

interface StatsPanelProps {
  stats: StatisticsSnapshot;
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

export function StatsPanel({ stats }: StatsPanelProps) {
  return (
    <aside className="stats-panel" aria-label="Log statistics">
      <dl className="stats-list">
        <div className="stats-item stats-item--error">
          <dt>Errors</dt>
          <dd>{stats.errors}</dd>
        </div>
        <div className="stats-item stats-item--warn">
          <dt>Warnings</dt>
          <dd>{stats.warnings}</dd>
        </div>
        <div className="stats-item">
          <dt>Logs/s</dt>
          <dd>{stats.logsPerSecond.toFixed(1)}</dd>
        </div>
        <div className="stats-item">
          <dt>Memory</dt>
          <dd>{formatMemory(stats.memoryBytes)}</dd>
        </div>
        <div className="stats-item">
          <dt>Hidden</dt>
          <dd>{stats.hidden}</dd>
        </div>
      </dl>
    </aside>
  );
}
