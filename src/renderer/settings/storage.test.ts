import { beforeEach, describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS, SETTINGS_STORAGE_KEY } from './types';
import { loadSettings, normalizeSettings, saveSettings } from './storage';

describe('settings storage', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('returns defaults for empty storage', () => {
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it('normalizes partial and invalid values', () => {
    const settings = normalizeSettings({
      columns: { time: false, pid: 'nope', message: false },
      levelColors: { error: 'red', info: '#0f0' },
      maxVisibleRows: 1234,
      locale: 'fr',
    });

    expect(settings.columns.time).toBe(false);
    expect(settings.columns.pid).toBe(true);
    expect(settings.columns.message).toBe(true);
    expect(settings.levelColors.error).toBe(DEFAULT_SETTINGS.levelColors.error);
    expect(settings.levelColors.info).toBe('#0f0');
    expect(settings.maxVisibleRows).toBe(1000);
    expect(settings.locale).toBe('zh');
  });

  it('round-trips settings through localStorage', () => {
    const next = {
      ...DEFAULT_SETTINGS,
      columns: { ...DEFAULT_SETTINGS.columns, tid: false },
      locale: 'en' as const,
      maxVisibleRows: 2000,
    };
    saveSettings(next);
    expect(localStorage.getItem(SETTINGS_STORAGE_KEY)).toContain('"tid":false');
    expect(loadSettings()).toEqual(next);
  });
});
