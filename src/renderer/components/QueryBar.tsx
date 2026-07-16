import type { LogLevel } from '../types/protocol';
import type { Locale } from '../settings/types';
import { t } from '../settings/i18n';

export const FILTER_LEVELS: Array<Exclude<LogLevel, 'unknown'>> = [
  'verbose',
  'debug',
  'info',
  'warn',
  'error',
  'assert',
];

export const DEFAULT_SELECTED_LEVELS: Array<Exclude<LogLevel, 'unknown'>> = [...FILTER_LEVELS];

const LEVEL_LABEL: Record<Exclude<LogLevel, 'unknown'>, string> = {
  verbose: 'V',
  debug: 'D',
  info: 'I',
  warn: 'W',
  error: 'E',
  assert: 'A',
};

export function composeFilterQuery(
  packageFilter: string,
  tagFilter: string,
  selectedLevels: ReadonlyArray<Exclude<LogLevel, 'unknown'>>,
): string {
  const parts: string[] = [];
  const pkg = packageFilter.trim();
  const tag = tagFilter.trim();
  if (pkg) parts.push(`package:${pkg}`);
  if (tag) parts.push(`tag:${tag}`);
  if (selectedLevels.length === 0) {
    parts.push('level:none');
  } else if (selectedLevels.length < FILTER_LEVELS.length) {
    for (const level of selectedLevels) {
      parts.push(`level:${level}`);
    }
  }
  return parts.join(' ');
}

interface QueryBarProps {
  packageFilter: string;
  tagFilter: string;
  selectedLevels: ReadonlyArray<Exclude<LogLevel, 'unknown'>>;
  onPackageChange: (value: string) => void;
  onTagChange: (value: string) => void;
  onLevelToggle: (level: Exclude<LogLevel, 'unknown'>) => void;
  locale: Locale;
}

export function QueryBar({
  packageFilter,
  tagFilter,
  selectedLevels,
  onPackageChange,
  onTagChange,
  onLevelToggle,
  locale,
}: QueryBarProps) {
  return (
    <div className="query-bar">
      <label className="field query-bar__field">
        <span className="field__label">{t(locale, 'package')}</span>
        <input
          className="field__input"
          value={packageFilter}
          onChange={(event) => onPackageChange(event.currentTarget.value)}
          placeholder="com.example.app"
          aria-label={t(locale, 'package')}
        />
      </label>
      <label className="field query-bar__field">
        <span className="field__label">{t(locale, 'tag')}</span>
        <input
          className="field__input"
          value={tagFilter}
          onChange={(event) => onTagChange(event.currentTarget.value)}
          placeholder="ActivityManager"
          aria-label={t(locale, 'tag')}
        />
      </label>
      <fieldset className="level-filters" aria-label={t(locale, 'level')}>
        <legend className="field__label">{t(locale, 'level')}</legend>
        <div className="level-filters__options">
          {FILTER_LEVELS.map((level) => {
            const checked = selectedLevels.includes(level);
            return (
              <label key={level} className={`level-chip level-chip--${level}${checked ? ' is-checked' : ''}`}>
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={() => onLevelToggle(level)}
                  aria-label={level}
                />
                <span>{LEVEL_LABEL[level]}</span>
              </label>
            );
          })}
        </div>
      </fieldset>
    </div>
  );
}
