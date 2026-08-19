// Selection + keyboard navigation for the file tree.

import { useCallback, useMemo, useState } from 'react';
import type { FileNode } from '../ipc';

export interface Selection {
  /** Flat, pre-order list of entries in the currently shown directory. */
  entries: FileNode[];
  /** Index of the focused entry, or null. */
  focusedIndex: number | null;
  /** Paths of the selected entries (single = focused entry). */
  selected: string[];
  /** Move the focus cursor. */
  move: (delta: number) => void;
  /** Focus the first entry (used when entering a directory). */
  focusFirst: () => void;
  /** Clear all selection. */
  clear: () => void;
}

/** Flatten a node's direct children into a stable list. */
export function flattenEntries(node: FileNode | null): FileNode[] {
  if (!node) return [];
  return node.children ?? [];
}

export function useSelection(entries: FileNode[]): Selection {
  const [focusedIndex, setFocusedIndex] = useState<number | null>(null);

  const move = useCallback(
    (delta: number) => {
      if (entries.length === 0) return;
      setFocusedIndex((prev) => {
        const base = prev ?? 0;
        const next = base + delta;
        if (next < 0) return 0;
        if (next >= entries.length) return entries.length - 1;
        return next;
      });
    },
    [entries.length],
  );

  const focusFirst = useCallback(() => {
    if (entries.length > 0) setFocusedIndex(0);
  }, [entries.length]);

  const clear = useCallback(() => setFocusedIndex(null), []);

  const selected = useMemo(() => {
    if (focusedIndex === null || focusedIndex >= entries.length) return [];
    return [entries[focusedIndex].path];
  }, [focusedIndex, entries]);

  return { entries, focusedIndex, selected, move, focusFirst, clear };
}
