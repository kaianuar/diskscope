import { useState, useCallback, useRef } from "react";
import { startScan } from "../ipc";
import type { ScanResult, ScanFilter, FileNode } from "../domain";

export interface ScanState {
  result: ScanResult | null;
  loading: boolean;
  error: string | null;
  /** The node currently displayed (may be a subdirectory). */
  currentNode: FileNode | null;
  /** Navigation breadcrumb stack. */
  breadcrumbs: FileNode[];
}

/**
 * Manages scan lifecycle: invoke scan, navigate into subdirectories, go back.
 */
export function useScan(_initialPath: string) {
  const [state, setState] = useState<ScanState>({
    result: null,
    loading: false,
    error: null,
    currentNode: null,
    breadcrumbs: [],
  });
  const cancelRef = useRef(false);

  const scan = useCallback(
    async (path: string, filter?: ScanFilter) => {
      setState((s) => ({ ...s, loading: true, error: null }));
      cancelRef.current = false;
      try {
        const result = await startScan(path, filter);
        if (cancelRef.current) return;
        setState({
          result,
          loading: false,
          error: null,
          currentNode: result.root_node,
          breadcrumbs: [],
        });
      } catch (err) {
        if (cancelRef.current) return;
        setState((s) => ({
          ...s,
          loading: false,
          error: String(err),
        }));
      }
    },
    []
  );

  /** Drill into a child directory node. */
  const drillInto = useCallback((node: FileNode) => {
    if (!node.is_dir) return;
    setState((s) => ({
      ...s,
      currentNode: node,
      breadcrumbs: [...(s.currentNode ? [s.currentNode] : []), ...s.breadcrumbs],
    }));
  }, []);

  /** Go up one level. */
  const goUp = useCallback(() => {
    setState((s) => {
      const [parent, ...rest] = s.breadcrumbs;
      if (!parent) return s;
      return { ...s, currentNode: parent, breadcrumbs: rest };
    });
  }, []);

  /** Navigate to a specific breadcrumb index. */
  const goToBreadcrumb = useCallback((index: number) => {
    setState((s) => {
      if (index >= s.breadcrumbs.length) return s;
      const target = s.breadcrumbs[index];
      const newCrumbs = s.breadcrumbs.slice(index + 1);
      return { ...s, currentNode: target, breadcrumbs: newCrumbs };
    });
  }, []);

  /** Cancel a running scan. */
  const cancel = useCallback(() => {
    cancelRef.current = true;
    setState((s) => ({ ...s, loading: false }));
  }, []);

  return { ...state, scan, drillInto, goUp, goToBreadcrumb, cancel };
}
