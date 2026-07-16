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

  it('splits package input on | for multi-package OR', () => {
    expect(composeFilterQuery('com.foo | com.bar', '', DEFAULT_SELECTED_LEVELS)).toBe(
      'package:com.foo package:com.bar',
    );
  });

  it('emits selected level tokens when subset is checked', () => {
    expect(composeFilterQuery('', '', ['warn', 'error'])).toBe('level:warn level:error');
  });

  it('emits level:none when no levels are checked', () => {
    expect(composeFilterQuery('pkg', '', [])).toBe('package:pkg level:none');
  });

  it('appends case:insensitive when enabled', () => {
    expect(composeFilterQuery('pkg', 'tag1|tag2', DEFAULT_SELECTED_LEVELS, true)).toBe(
      'package:pkg tag:tag1 tag:tag2 case:insensitive',
    );
  });

  it('splits tag input on | for multi-tag OR', () => {
    expect(composeFilterQuery('', 'tag1 | tag2 | tag3', DEFAULT_SELECTED_LEVELS)).toBe(
      'tag:tag1 tag:tag2 tag:tag3',
    );
    expect(composeFilterQuery('', 'ActivityManager|Choreographer', DEFAULT_SELECTED_LEVELS)).toBe(
      'tag:ActivityManager tag:Choreographer',
    );
  });
});
