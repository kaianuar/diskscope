import { describe, expect, it } from 'vitest';
import { parentOf } from '../../lib/pathUtils';

describe('parentOf', () => {
  it('should return parent for POSIX path', () => {
    expect(parentOf('/a/b')).toBe('/a');
  });

  it('should return parent for nested POSIX path', () => {
    expect(parentOf('/home/user/projects/src')).toBe('/home/user/projects');
  });

  it('should return root for single POSIX segment', () => {
    expect(parentOf('/a')).toBe('/');
  });

  it('should return root for bare root', () => {
    expect(parentOf('/')).toBe('/');
  });

  it('should return parent for Windows path', () => {
    expect(parentOf('C:\\a\\b')).toBe('C:\\a');
  });

  it('should return drive root for single Windows segment', () => {
    expect(parentOf('C:\\a')).toBe('C:\\');
  });

  it('should handle trailing POSIX separator', () => {
    expect(parentOf('/a/b/')).toBe('/a');
  });

  it('should handle trailing Windows separator', () => {
    expect(parentOf('C:\\a\\b\\')).toBe('C:\\a');
  });
});
