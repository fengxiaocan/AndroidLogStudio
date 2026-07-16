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
  caseInsensitive: boolean = false,
): string {
  const parts: string[] = [];
  const pkg = packageFilter.trim();
  const tag = tagFilter.trim();
  if (pkg) {
    // Support multi-package OR with |
    const pkgParts = pkg
      .split('|')
      .map((p) => p.trim())
      .filter(Boolean);
    for (const p of pkgParts) {
      parts.push(`package:${p}`);
    }
  }
  if (tag) {
    // Support multi-tag OR with | , e.g. "tag1 | tag2 | tag3"
    const tagParts = tag
      .split('|')
      .map((p) => p.trim())
      .filter(Boolean);
    for (const p of tagParts) {
      parts.push(`tag:${p}`);
    }
  }
  if (selectedLevels.length === 0) {
    parts.push('level:none');
  } else if (selectedLevels.length < FILTER_LEVELS.length) {
    for (const level of selectedLevels) {
      parts.push(`level:${level}`);
    }
  }
  if (caseInsensitive) {
    parts.push('case:insensitive');
  }
  return parts.join(' ');
}

interface QueryBarProps {
  packageFilter: string;
  tagFilter: string;
  selectedLevels: ReadonlyArray<Exclude<LogLevel, 'unknown'>>;
  caseInsensitive: boolean;
  onPackageChange: (value: string) => void;
  onTagChange: (value: string) => void;
  onLevelToggle: (level: Exclude<LogLevel, 'unknown'>) => void;
  onCaseInsensitiveChange: (value: boolean) => void;
  locale: Locale;
}

export function QueryBar({
  packageFilter,
  tagFilter,
  selectedLevels,
  caseInsensitive,
  onPackageChange,
  onTagChange,
  onLevelToggle,
  onCaseInsensitiveChange,
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
          placeholder={t(locale, 'tagPlaceholder')}
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

          <label className="case-toggle">
            <input
              type="checkbox"
              checked={caseInsensitive}
              onChange={(event) => onCaseInsensitiveChange(event.currentTarget.checked)}
            />
            <span>{t(locale, 'caseInsensitive')}</span>
          </label>
        </div>
      </fieldset>
    </div>
  );
}
