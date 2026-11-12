import { describe, expect, it } from 'vitest';
import { activeWorkspace } from './state';

describe('shell state', () => {
  it('starts with no active workspace', () => {
    expect(activeWorkspace.value).toBeNull();
  });
});
