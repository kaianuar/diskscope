import { describe, expect, it } from 'vitest';
import { formatSize } from '../../lib/formatSize';

describe('formatSize', () => {
  it('should return "0 B" when bytes is 0', () => {
    expect(formatSize(0)).toBe('0 B');
  });

  it('should return "0 B" when bytes is negative', () => {
    expect(formatSize(-100)).toBe('0 B');
  });

  it('should return "0 B" when bytes is NaN', () => {
    expect(formatSize(NaN)).toBe('0 B');
  });

  it('should return "0 B" when bytes is Infinity', () => {
    expect(formatSize(Infinity)).toBe('0 B');
  });

  it('should return exact bytes when under 1024', () => {
    expect(formatSize(1)).toBe('1 B');
    expect(formatSize(512)).toBe('512 B');
    expect(formatSize(1023)).toBe('1023 B');
  });

  it('should format KiB when bytes is 1024', () => {
    expect(formatSize(1024)).toBe('1.0 KiB');
  });

  it('should format MiB when bytes is in megabyte range', () => {
    expect(formatSize(1024 * 1024)).toBe('1.0 MiB');
  });

  it('should format GiB when bytes is in gigabyte range', () => {
    expect(formatSize(1024 * 1024 * 1024)).toBe('1.0 GiB');
  });

  it('should format TiB when bytes is in terabyte range', () => {
    expect(formatSize(1024 ** 4)).toBe('1.0 TiB');
  });

  it('should format PiB when bytes is in petabyte range', () => {
    expect(formatSize(1024 ** 5)).toBe('1.0 PiB');
  });

  it('should show one decimal place for non-exact units', () => {
    expect(formatSize(1536)).toBe('1.5 KiB');
    expect(formatSize(1572864)).toBe('1.5 MiB');
  });

  it('should not exceed PiB unit', () => {
    expect(formatSize(1024 ** 6)).toMatch(/PiB$/);
  });
});
