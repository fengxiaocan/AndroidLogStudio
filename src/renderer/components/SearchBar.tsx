import type { Locale } from '../settings/types';
import { t } from '../settings/i18n';

interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  locale: Locale;
}

export function SearchBar({ value, onChange, locale }: SearchBarProps) {
  return (
    <div className="search-bar">
      <input
        className="field__input"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
        placeholder={t(locale, 'search')}
        aria-label={t(locale, 'search')}
      />
    </div>
  );
}
