// Scan lifecycle hook: start/cancel scans, track progress, expose the
// finished result. IPC calls go through `ipc.ts` so tests can mock the
// module.

import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { Filter, ScanResult } from '../ipc';
import { cancelScan, startScan } from '../ipc';

export interface UseScanResult {
  /** Current scan id, or null when idle. */
  scanId: number | null;
  /** 0-100 progress of the running scan, or null when not scanning. */
  progress: number | null;
  /** The finished scan result, or null before the first scan completes. */
  result: ScanResult | null;
  /** Human-readable error from the last failed operation. */
  error: string | null;
  /** Start a scan; replaces any previous result. */
  start: (path: string, filter?: Filter) => Promise<void>;
  /** Cancel the running scan. */
  cancel: () => Promise<void>;
  /** Set the error string directly (used by action handlers). */
  setError: (message: string | null) => void;
}

export function useScan(): UseScanResult {
  const [scanId, setScanId] = useState<number | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [result, setResult] = useState<ScanResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Live `scan-progress` / `scan-done` events.
  useEffect(() => {
    let unlisten: Array<() => void> = [];
    let cancelled = false;
    (async () => {
      const unP = await listen<number>('scan-progress', (e) => {
        if (!cancelled) setProgress(e.payload);
      });
      const unD = await listen<{ error?: string }>('scan-done', (e) => {
        if (cancelled) return;
        if (e.payload && typeof e.payload.error === 'string') {
          setError(e.payload.error);
        } else {
          setError(null);
        }
        setProgress(null);
      });
      if (!cancelled) unlisten = [unP, unD];
    })();
    return () => {
      cancelled = true;
      unlisten.forEach((fn) => fn());
    };
  }, []);

  const start = useCallback(async (path: string, filter?: Filter) => {
    setError(null);
    const id = await startScan(path, filter);
    setScanId(id);
    setProgress(0);
  }, []);

  const cancel = useCallback(async () => {
    if (scanId === null) return;
    await cancelScan(scanId);
    setScanId(null);
    setProgress(null);
  }, [scanId]);

  return { scanId, progress, result, error, start, cancel, setError };
}
