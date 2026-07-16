import { describe, expect, it } from 'vitest';
import { composeFilterQuery, DEFAULT_SELECTED_LEVELS } from './QueryBar';

describe('composeFilterQuery', () => {
  it('returns empty query when nothing is filtered and all levels selected', () => {
    expect(composeFilterQuery('', '', DEFAULT_SELECTED_LEVELS)).toBe('');
  });

  it('composes package and tag tokens', () => {
    expect(composeFilterQuery('com.example', 'ActivityManager', DEFAULT_SELECTED_LEVELS)).toBe(
      'package:com.example tag:ActivityManager',
    );
  });

  it('emits selected level tokens when subset is checked', () => {
    expect(composeFilterQuery('', '', ['warn', 'error'])).toBe('level:warn level:error');
  });

  it('emits level:none when no levels are checked', () => {
    expect(composeFilterQuery('pkg', '', [])).toBe('package:pkg level:none');
  });
});
