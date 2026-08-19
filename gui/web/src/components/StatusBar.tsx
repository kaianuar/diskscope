// Status bar: scan summary, current path, errors.

import { formatSize } from '../lib/formatSize';
import type { ScanResult } from '../ipc';

export interface StatusBarProps {
  result: ScanResult | null;
  error: string | null;
  path: string | null;
}

export function StatusBar({ result, error, path }: StatusBarProps) {
  return (
    <div className="status-bar" data-testid="status-bar">
      {error ? (
        <span className="status-error" data-testid="scan-error">
          {error}
        </span>
      ) : (
        <>
          <span data-testid="status-path">{path ?? 'No scan yet'}</span>
          {result && (
            <span data-testid="status-summary">
              {formatSize(result.totalSize)} · {result.fileCount.toLocaleString()} entries ·{' '}
              {result.scanDurationMs} ms{result.skipped.length > 0 ? ` · ${result.skipped.length} skipped` : ''}
            </span>
          )}
        </>
      )}
    </div>
  );
}
