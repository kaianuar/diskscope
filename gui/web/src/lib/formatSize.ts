// Human-readable byte formatting, mirroring `domain::format_size`
// (binary IEC units).

const UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'] as const;

export function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0) return `${bytes} B`;
  return `${value.toFixed(1)} ${UNITS[unit]}`;
}
