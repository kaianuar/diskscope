// DiskScope app shell: toolbar, sidebar, filter panel, treemap, table,
// status bar. Owns scan state and wires keyboard shortcuts + context
// menu actions.

import { useCallback, useMemo, useState } from 'react';
import { useScan } from './hooks/useScan';
import { useSelection } from './hooks/useSelection';
import { useShortcuts } from './hooks/useShortcuts';
import { deletePaths, undoLastDelete, type Filter, type SortColumn, type SortDirection } from './ipc';
import { TreemapCanvas2D } from './components/TreemapCanvas2D';
import { TableView } from './components/TableView';
import { Toolbar } from './components/Toolbar';
import { Sidebar } from './components/Sidebar';
import { FilterPanel } from './components/FilterPanel';
import { StatusBar } from './components/StatusBar';
import { ContextMenu } from './components/ContextMenu';
import { Breadcrumb } from './components/Breadcrumb';

// Quick-scan shortcuts: OS-aware home dir + root.
function homeQuickPaths(): { label: string; path: string }[] {
  const home =
    (typeof process !== 'undefined' && process.env?.HOME) ||
    (typeof process !== 'undefined' && process.env?.USERPROFILE) ||
    '';
  const paths: { label: string; path: string }[] = [];
  if (home) paths.push({ label: 'Home', path: home });
  paths.push({ label: 'Root', path: '/' });
  return paths;
}

export function App() {
  const scan = useScan();
  const [currentPath, setCurrentPath] = useState<string | null>(null);
  const [sortColumn, setSortColumn] = useState<SortColumn | null>('size');
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc');
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  const [filter, setFilter] = useState<Filter | undefined>(undefined);
  const [ctxMenu, setCtxMenu] = useState<{ path: string; x: number; y: number } | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const [histIndex, setHistIndex] = useState(-1);
  const quickPaths = useMemo(homeQuickPaths, []);

  const currentEntries = useMemo(() => {
    const root = scan.result?.root ?? null;
    if (!root) return [];
    if (!currentPath || currentPath === root.path) return root.children ?? [];
    // Find the directory node matching the current path.
    const stack = [root];
    while (stack.length > 0) {
      const node = stack.pop();
      if (!node) continue;
      if (node.path === currentPath) return node.children ?? [];
      stack.push(...(node.children ?? []));
    }
    return root.children ?? [];
  }, [scan.result, currentPath]);

  const selection = useSelection(currentEntries);

  const navigateTo = useCallback(
    (path: string) => {
      if (path === currentPath) return;
      setCurrentPath(path);
      // Push onto history, truncating any forward entries first.
      setHistory((h) => [...h.slice(0, histIndex + 1), path]);
      setHistIndex((i) => i + 1);
      selection.focusFirst();
    },
    [selection, currentPath, histIndex],
  );

  const goBack = useCallback(() => {
    setHistIndex((i) => {
      if (i > 0) {
        const next = i - 1;
        setCurrentPath(history[next]);
        return next;
      }
      return i;
    });
  }, [history]);

  const goForward = useCallback(() => {
    setHistIndex((i) => {
      if (i < history.length - 1) {
        const next = i + 1;
        setCurrentPath(history[next]);
        return next;
      }
      return i;
    });
  }, [history]);

  const goUp = useCallback(() => {
    if (!scan.result) return;
    const rootPath = scan.result.root.path;
    if (!currentPath || currentPath === rootPath) return;
    const sep = currentPath.includes('\\') ? '\\' : '/';
    const parent = currentPath.slice(0, currentPath.lastIndexOf(sep));
    navigateTo(parent.length > 0 ? parent : rootPath);
  }, [currentPath, scan.result, navigateTo]);

  const handleActivate = useCallback(
    (entry: { node: { fileType: string; path: string } }) => {
      if (entry.node.fileType === 'directory') navigateTo(entry.node.path);
    },
    [navigateTo],
  );

  const handleDelete = useCallback(async () => {
    if (selection.selected.length === 0) return;
    try {
      await deletePaths(selection.selected);
      selection.clear();
    } catch (err) {
      scan.setError(String(err));
    }
  }, [selection, scan]);

  const handleUndo = useCallback(async () => {
    try {
      await undoLastDelete();
    } catch (err) {
      scan.setError(String(err));
    }
  }, [scan]);

  const handleSort = useCallback(
    (column: SortColumn) => {
      if (sortColumn === column) {
        setSortDirection((d) => (d === 'asc' ? 'desc' : 'asc'));
      } else {
        setSortColumn(column);
        setSortDirection('desc');
      }
    },
    [sortColumn],
  );

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, path: string) => {
      e.preventDefault();
      setCtxMenu({ path, x: e.clientX, y: e.clientY });
    },
    [],
  );

  useShortcuts({
    onMove: selection.move,
    onEnter: () => {
      const idx = selection.focusedIndex;
      if (idx !== null && currentEntries[idx]?.fileType === 'directory') {
        navigateTo(currentEntries[idx].path);
      }
    },
    onBack: () => {
      if (!scan.result) return;
      const rootPath = scan.result.root.path;
      if (!currentPath || currentPath === rootPath) return;
      const parent = currentPath.slice(0, currentPath.lastIndexOf('/'));
      setCurrentPath(parent.length > 0 ? parent : rootPath);
    },
    onDelete: () => void handleDelete(),
    onUndo: () => void handleUndo(),
  });

  return (
    <div className="app-shell" data-testid="app-shell">
      <div className="app-sidebar">
        <Sidebar
          onScan={(p) => void scan.start(p, filter)}
          scanning={scan.progress !== null}
          quickPaths={quickPaths}
        />
      </div>
      <div className="app-main">
        <Toolbar
          scanning={scan.progress !== null}
          progress={scan.progress}
          onCancel={() => void scan.cancel()}
          onRescan={() => void scan.start(currentPath ?? scan.result?.root.path ?? '/', filter)}
          canGoUp={!!currentPath && !!scan.result && currentPath !== scan.result.root.path}
          onGoUp={goUp}
          canGoBack={histIndex > 0}
          onGoBack={goBack}
          canGoForward={histIndex < history.length - 1}
          onGoForward={goForward}
        />
        <Breadcrumb
          path={currentPath}
          rootPath={scan.result?.root.path ?? '/'}
          onNavigate={navigateTo}
        />
        <FilterPanel value={filter} onChange={setFilter} />
        <div className="app-content">
          <TreemapCanvas2D
            root={scan.result?.root ?? { path: '', size: 0, modified: 0, fileType: 'directory', children: [] }}
            hoveredIndex={hoveredIndex}
            onHover={setHoveredIndex}
            onActivate={handleActivate}
          />
          <TableView
            entries={currentEntries}
            sortColumn={sortColumn}
            sortDirection={sortDirection}
            onSort={handleSort}
            onActivate={(entry) => handleActivate({ node: entry })}
            onContextMenu={handleContextMenu}
          />
        </div>
        <StatusBar
          result={scan.result}
          error={scan.error}
          path={currentPath ?? scan.result?.root.path ?? null}
        />
      </div>
      {ctxMenu && scan.result && (
        <ContextMenu
          path={ctxMenu.path}
          rootPath={scan.result.root.path}
          x={ctxMenu.x}
          y={ctxMenu.y}
          onClose={() => setCtxMenu(null)}
          onError={scan.setError}
        />
      )}
    </div>
  );
}
