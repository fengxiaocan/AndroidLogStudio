import { describe, expect, it } from 'vitest';
import { buildExportFileName } from './fileName';

describe('buildExportFileName', () => {
  it('sanitizes device label and includes mode', () => {
    const name = buildExportFileName('Mock Device', 'filtered', new Date('2026-07-16T12:34:56'));
    expect(name).toMatch(/^Mock_Device-filtered-\d{8}-\d{6}\.log$/);
  });
});
