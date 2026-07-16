import type { CSSProperties } from 'react';
import { Virtuoso } from 'react-virtuoso';
import type { AppSettings } from '../settings/types';
import type { LogEntry } from '../types/protocol';

interface LogViewProps {
  logs: LogEntry[];
  searchQuery: string;
  settings: AppSettings;
}

function levelLabel(level: LogEntry['level']) {
  return level.charAt(0).toUpperCase();
}

function highlightFirstMatch(message: string, searchQuery: string) {
  if (!searchQuery.trim()) {
    return message;
  }

  const matchIndex = message.toLocaleLowerCase().indexOf(searchQuery.toLocaleLowerCase());
  if (matchIndex === -1) {
    return message;
  }

  const before = message.slice(0, matchIndex);
  const match = message.slice(matchIndex, matchIndex + searchQuery.length);
  const after = message.slice(matchIndex + searchQuery.length);

  return (
    <>
      {before}
      <mark>{match}</mark>
      {after}
    </>
  );
}

function buildGridTemplate(columns: AppSettings['columns']): string {
  const parts: string[] = [];
  if (columns.time) parts.push('88px');
  if (columns.pid) parts.push('56px');
  if (columns.tid) parts.push('48px');
  if (columns.level) parts.push('28px');
  if (columns.package) parts.push('minmax(100px, 180px)');
  if (columns.tag) parts.push('minmax(100px, 180px)');
  if (columns.message) parts.push('minmax(0, 1fr)');
  return parts.join(' ') || 'minmax(0, 1fr)';
}

function levelColorStyle(settings: AppSettings): CSSProperties {
  return {
    ['--level-verbose' as string]: settings.levelColors.verbose,
    ['--level-debug' as string]: settings.levelColors.debug,
    ['--level-info' as string]: settings.levelColors.info,
    ['--level-warn' as string]: settings.levelColors.warn,
    ['--level-error' as string]: settings.levelColors.error,
    ['--level-assert' as string]: settings.levelColors.assert,
  };
}

export function LogView({ logs, searchQuery, settings }: LogViewProps) {
  const gridTemplate = buildGridTemplate(settings.columns);
  const { columns } = settings;

  return (
    <section className="log-view" aria-label="Log output" style={levelColorStyle(settings)}>
      <Virtuoso
        className="log-list"
        data={logs}
        followOutput="smooth"
        itemContent={(_, log) => (
          <div
            className={`log-row log-row--${log.level}`}
            style={{ gridTemplateColumns: gridTemplate }}
          >
            {columns.time ? <span className="log-row__time">{log.time}</span> : null}
            {columns.pid ? (
              <span className="log-row__pid" title={`pid ${log.pid}`}>
                {log.pid}
              </span>
            ) : null}
            {columns.tid ? <span className="log-row__tid">{log.tid}</span> : null}
            {columns.level ? <span className="log-row__level">{levelLabel(log.level)}</span> : null}
            {columns.package ? (
              <span className="log-row__package" title={log.packageName ?? undefined}>
                {log.packageName ?? '—'}
              </span>
            ) : null}
            {columns.tag ? <span className="log-row__tag">{log.tag}</span> : null}
            {columns.message ? (
              <span className="log-row__message">{highlightFirstMatch(log.message, searchQuery)}</span>
            ) : null}
          </div>
        )}
        style={{ height: '100%' }}
      />
    </section>
  );
}
