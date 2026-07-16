import {
  DEFAULT_SETTINGS,
  MAX_VISIBLE_ROWS_OPTIONS,
  SETTINGS_STORAGE_KEY,
  type AppSettings,
  type ColumnVisibility,
  type LevelColors,
  type Locale,
} from './types';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function asBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === 'boolean' ? value : fallback;
}

function asColor(value: unknown, fallback: string): string {
  if (typeof value !== 'string') return fallback;
  const trimmed = value.trim();
  if (/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(trimmed)) {
    return trimmed;
  }
  return fallback;
}

function asLocale(value: unknown): Locale {
  return value === 'en' || value === 'zh' ? value : DEFAULT_SETTINGS.locale;
}

function asMaxVisibleRows(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return DEFAULT_SETTINGS.maxVisibleRows;
  }
  const rounded = Math.round(value);
  if ((MAX_VISIBLE_ROWS_OPTIONS as readonly number[]).includes(rounded)) {
    return rounded;
  }
  // Clamp to nearest supported option.
  return MAX_VISIBLE_ROWS_OPTIONS.reduce((best, option) =>
    Math.abs(option - rounded) < Math.abs(best - rounded) ? option : best,
  );
}

function mergeColumns(raw: unknown): ColumnVisibility {
  const base = { ...DEFAULT_SETTINGS.columns };
  if (!isRecord(raw)) return base;
  return {
    time: asBoolean(raw.time, base.time),
    pid: asBoolean(raw.pid, base.pid),
    tid: asBoolean(raw.tid, base.tid),
    level: asBoolean(raw.level, base.level),
    package: asBoolean(raw.package, base.package),
    tag: asBoolean(raw.tag, base.tag),
    // Message column must stay visible so rows remain readable.
    message: true,
  };
}

function mergeLevelColors(raw: unknown): LevelColors {
  const base = { ...DEFAULT_SETTINGS.levelColors };
  if (!isRecord(raw)) return base;
  return {
    verbose: asColor(raw.verbose, base.verbose),
    debug: asColor(raw.debug, base.debug),
    info: asColor(raw.info, base.info),
    warn: asColor(raw.warn, base.warn),
    error: asColor(raw.error, base.error),
    assert: asColor(raw.assert, base.assert),
  };
}

export function normalizeSettings(raw: unknown): AppSettings {
  if (!isRecord(raw)) {
    return {
      columns: { ...DEFAULT_SETTINGS.columns },
      levelColors: { ...DEFAULT_SETTINGS.levelColors },
      maxVisibleRows: DEFAULT_SETTINGS.maxVisibleRows,
      locale: DEFAULT_SETTINGS.locale,
    };
  }

  return {
    columns: mergeColumns(raw.columns),
    levelColors: mergeLevelColors(raw.levelColors),
    maxVisibleRows: asMaxVisibleRows(raw.maxVisibleRows),
    locale: asLocale(raw.locale),
  };
}

export function loadSettings(): AppSettings {
  try {
    if (typeof localStorage === 'undefined') {
      return normalizeSettings(null);
    }
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) {
      return normalizeSettings(null);
    }
    return normalizeSettings(JSON.parse(raw) as unknown);
  } catch {
    return normalizeSettings(null);
  }
}

export function saveSettings(settings: AppSettings): void {
  try {
    if (typeof localStorage === 'undefined') return;
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Ignore quota / private mode write failures.
  }
}
