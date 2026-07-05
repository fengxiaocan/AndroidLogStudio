import { Virtuoso } from 'react-virtuoso';
import type { LogEntry } from '../types/protocol';

interface LogViewProps {
  logs: LogEntry[];
  searchQuery: string;
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

export function LogView({ logs, searchQuery }: LogViewProps) {
  return (
    <section className="log-view" aria-label="Log output">
      <Virtuoso
        className="log-list"
        data={logs}
        followOutput="smooth"
        itemContent={(_, log) => (
          <div className={`log-row log-row--${log.level}`}>
            <span className="log-row__time">{log.time}</span>
            <span className="log-row__pid">{log.pid}</span>
            <span className="log-row__tid">{log.tid}</span>
            <span className="log-row__level">{levelLabel(log.level)}</span>
            <span className="log-row__tag">{log.tag}</span>
            <span className="log-row__message">{highlightFirstMatch(log.message, searchQuery)}</span>
          </div>
        )}
        style={{ height: '100%' }}
      />
    </section>
  );
}
