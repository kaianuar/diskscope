// Tests for the scan + selection hooks.

import { describe, expect, it, vi } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';

// Mock the IPC boundary so hook tests never touch Tauri.
vi.mock('../../ipc', () => ({
  startScan: vi.fn(async () => 42),
  cancelScan: vi.fn(async () => 42),
  deletePaths: vi.fn(async () => undefined),
  undoLastDelete: vi.fn(async () => undefined),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => undefined),
}));

import { useScan } from '../useScan';
import { useSelection, flattenEntries } from '../useSelection';
import { startScan, cancelScan } from '../../ipc';
import type { FileNode } from '../../ipc';

function fileNode(path: string, size: number): FileNode {
  return { path, size, modified: 0, fileType: 'other', children: [] };
}

describe('useScan', () => {
  it('should dispatch scan IPC when StartScan invoked', async () => {
    const { result } = renderHook(() => useScan());
    await act(async () => {
      await result.current.start('/home');
    });
    expect(startScan).toHaveBeenCalledWith('/home', undefined);
    expect(result.current.scanId).toBe(42);
    expect(result.current.progress).toBe(0);
  });

  it('should cancel running scan when CancelScan invoked', async () => {
    const { result } = renderHook(() => useScan());
    await act(async () => {
      await result.current.start('/home');
    });
    expect(result.current.scanId).toBe(42);
    await act(async () => {
      await result.current.cancel();
    });
    expect(cancelScan).toHaveBeenCalledWith(42);
    expect(result.current.scanId).toBeNull();
    expect(result.current.progress).toBeNull();
  });
});

describe('useSelection', () => {
  it('should move focus and expose selected paths', () => {
    const entries = [fileNode('/a', 1), fileNode('/b', 2), fileNode('/c', 3)];
    const { result } = renderHook(() => useSelection(entries));
    expect(result.current.selected).toEqual([]);
    act(() => result.current.move(1));
    expect(result.current.focusedIndex).toBe(1);
    expect(result.current.selected).toEqual(['/b']);
    act(() => result.current.move(1));
    expect(result.current.focusedIndex).toBe(2);
    // Clamps at the end.
    act(() => result.current.move(5));
    expect(result.current.focusedIndex).toBe(2);
    act(() => result.current.clear());
    expect(result.current.selected).toEqual([]);
  });

  it('should flatten a node into its direct children', () => {
    const root: FileNode = {
      path: '/',
      size: 0,
      modified: 0,
      fileType: 'directory',
      children: [fileNode('/a', 1), fileNode('/b', 2)],
    };
    expect(flattenEntries(root)).toHaveLength(2);
    expect(flattenEntries(null)).toEqual([]);
  });
});
