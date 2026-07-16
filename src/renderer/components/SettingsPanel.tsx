import { useEffect, useId, useRef } from 'react';
import { columnLabel, levelLabel, t } from '../settings/i18n';
import {
  COLUMN_ORDER,
  LEVEL_COLOR_ORDER,
  MAX_VISIBLE_ROWS_OPTIONS,
  type Locale,
  type LogColumnId,
} from '../settings/types';
import { useAppStore } from '../state/appStore';

export function SettingsPanel() {
  const open = useAppStore((state) => state.settingsOpen);
  const settings = useAppStore((state) => state.settings);
  const closeSettings = useAppStore((state) => state.closeSettings);
  const setColumnVisible = useAppStore((state) => state.setColumnVisible);
  const setLevelColor = useAppStore((state) => state.setLevelColor);
  const setMaxVisibleRows = useAppStore((state) => state.setMaxVisibleRows);
  const setLocale = useAppStore((state) => state.setLocale);
  const resetSettings = useAppStore((state) => state.resetSettings);
  const titleId = useId();
  const closeRef = useRef<HTMLButtonElement>(null);
  const locale = settings.locale;

  useEffect(() => {
    if (!open) return;
    closeRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        closeSettings();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [open, closeSettings]);

  if (!open) {
    return null;
  }

  return (
    <div className="settings-overlay" role="presentation" onClick={closeSettings}>
      <div
        className="settings-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="settings-panel__header">
          <h2 id={titleId}>{t(locale, 'settingsTitle')}</h2>
          <button
            ref={closeRef}
            className="toolbar-btn"
            type="button"
            onClick={closeSettings}
            aria-label={t(locale, 'close')}
          >
            {t(locale, 'close')}
          </button>
        </header>

        <div className="settings-panel__body">
          <section className="settings-section" aria-label={t(locale, 'columnsSection')}>
            <h3>{t(locale, 'columnsSection')}</h3>
            <div className="settings-grid">
              {COLUMN_ORDER.map((column) => {
                const disabled = column === 'message';
                return (
                  <label key={column} className="settings-check">
                    <input
                      type="checkbox"
                      checked={settings.columns[column]}
                      disabled={disabled}
                      onChange={(event) => setColumnVisible(column as LogColumnId, event.currentTarget.checked)}
                    />
                    <span>{columnLabel(locale, column)}</span>
                  </label>
                );
              })}
            </div>
          </section>

          <section className="settings-section" aria-label={t(locale, 'colorsSection')}>
            <h3>{t(locale, 'colorsSection')}</h3>
            <div className="settings-color-grid">
              {LEVEL_COLOR_ORDER.map((level) => (
                <label key={level} className="settings-color">
                  <span>{levelLabel(locale, level)}</span>
                  <input
                    type="color"
                    value={settings.levelColors[level]}
                    onChange={(event) => setLevelColor(level, event.currentTarget.value)}
                    aria-label={levelLabel(locale, level)}
                  />
                </label>
              ))}
            </div>
          </section>

          <section className="settings-section" aria-label={t(locale, 'displaySection')}>
            <h3>{t(locale, 'displaySection')}</h3>
            <label className="field">
              <span className="field__label">{t(locale, 'maxVisibleRows')}</span>
              <select
                className="field__input"
                value={settings.maxVisibleRows}
                onChange={(event) => setMaxVisibleRows(Number(event.currentTarget.value))}
              >
                {MAX_VISIBLE_ROWS_OPTIONS.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            </label>
          </section>

          <section className="settings-section" aria-label={t(locale, 'languageSection')}>
            <h3>{t(locale, 'languageSection')}</h3>
            <label className="field">
              <span className="field__label">{t(locale, 'language')}</span>
              <select
                className="field__input"
                value={settings.locale}
                onChange={(event) => setLocale(event.currentTarget.value as Locale)}
              >
                <option value="zh">{t(locale, 'locale.zh')}</option>
                <option value="en">{t(locale, 'locale.en')}</option>
              </select>
            </label>
          </section>
        </div>

        <footer className="settings-panel__footer">
          <button className="toolbar-btn" type="button" onClick={resetSettings}>
            {t(locale, 'resetDefaults')}
          </button>
        </footer>
      </div>
    </div>
  );
}
