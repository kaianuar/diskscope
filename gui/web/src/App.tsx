// DiskScope app shell: toolbar, sidebar, filter panel, treemap, table,
// status bar. Owns scan state and wires keyboard shortcuts + context
// menu actions.

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useScan } from './hooks/useScan';
import { useSelection } from './hooks/useSelection';
import { useShortcuts } from './hooks/useShortcuts';
import { deletePaths, findDuplicates, openFile, revealInExplorer, undoLastDelete, type DuplicateReport, type Filter, type SortColumn, type SortDirection } from './ipc';
import { parentOf } from './lib/pathUtils';
import { TreemapCanvas2D } from './components/TreemapCanvas2D';
import { TableView } from './components/TableView';
import { Toolbar, type ThemeName } from './components/Toolbar';
import { Sidebar } from './components/Sidebar';
import { FilterPanel } from './components/FilterPanel';
import { StatusBar } from './components/StatusBar';
import { ContextMenu } from './components/ContextMenu';
import { Breadcrumb } from './components/Breadcrumb';
import { DuplicatesView } from './components/DuplicatesView';

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
  const [actionableIndex, setActionableIndex] = useState<number | null>(null);
  const [filter, setFilter] = useState<Filter | undefined>(undefined);
  const [ctxMenu, setCtxMenu] = useState<{ path: string; x: number; y: number } | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const [histIndex, setHistIndex] = useState(-1);
  const [theme, setTheme] = useState<ThemeName>(() => {
    const saved = localStorage.getItem('diskscope-theme');
    return saved === 'light' ? 'light' : 'dark';
  });
  const [view, setView] = useState<'files' | 'duplicates'>('files');
  const [dupeReport, setDupeReport] = useState<DuplicateReport | null>(null);
  const [dupesLoading, setDupesLoading] = useState(false);
  const [dupesError, setDupesError] = useState<string | null>(null);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('diskscope-theme', theme);
  }, [theme]);

  const toggleTheme = useCallback(() => {
    setTheme((t) => (t === 'dark' ? 'light' : 'dark'));
  }, []);

  const openDuplicates = useCallback(async () => {
    setView('duplicates');
    if (!dupeReport) {
      setDupesLoading(true);
      setDupesError(null);
      try {
        const report = await findDuplicates();
        setDupeReport(report);
      } catch (err) {
        setDupesError(String(err));
      } finally {
        setDupesLoading(false);
      }
    }
  }, [dupeReport]);

  const closeDuplicates = useCallback(() => {
    setView('files');
  }, []);

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
    const parent = parentOf(currentPath);
    navigateTo(parent.length > 0 ? parent : rootPath);
  }, [currentPath, scan.result, navigateTo]);

  const handleTreemapHover = useCallback(
    (index: number | null) => {
      setHoveredIndex(index);
      setActionableIndex(index);
    },
    [],
  );

  const handleActivate = useCallback(
    (entry: { node: { fileType: string; path: string } }) => {
      if (entry.node.fileType === 'directory') navigateTo(entry.node.path);
    },
    [navigateTo],
  );

  const handleOpen = useCallback(
    (entry: { node: { path: string } }) => {
      openFile(entry.node.path).catch((err) => scan.setError(String(err)));
    },
    [scan],
  );

  // The treemap always lays out the ROOT's children, so actionableIndex is
  // an index into `root.children` regardless of the current directory.
  const actionableHint = useMemo(() => {
    if (actionableIndex === null) return null;
    const entry = scan.result?.root.children?.[actionableIndex];
    if (!entry) return null;
    return entry.fileType === 'directory' ? 'Click to enter' : 'Click to open file';
  }, [actionableIndex, scan.result]);

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
      const parent = parentOf(currentPath);
      setCurrentPath(parent.length > 0 ? parent : rootPath);
    },
    onDelete: () => void handleDelete(),
    onUndo: () => void handleUndo(),
  });

  return (
    <div className="app-shell" data-testid="app-shell">
      <div className="app-sidebar">
        <Sidebar
          onScan={(p) => {
            setView('files');
            setDupeReport(null);
            void scan.start(p, filter);
          }}
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
          theme={theme}
          onToggleTheme={toggleTheme}
          canShowDuplicates={!!scan.result}
          onShowDuplicates={() => void openDuplicates()}
        />
        <Breadcrumb
          path={currentPath}
          rootPath={scan.result?.root.path ?? '/'}
          onNavigate={navigateTo}
        />
        <FilterPanel value={filter} onChange={setFilter} />
        {view === 'files' ? (
          <div className="app-content">
            <TreemapCanvas2D
              root={scan.result?.root ?? { path: '', size: 0, modified: 0, fileType: 'directory', children: [] }}
              hoveredIndex={hoveredIndex}
              actionableIndex={actionableIndex}
              onHover={handleTreemapHover}
              onActivate={handleActivate}
              onOpen={handleOpen}
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
        ) : (
          <DuplicatesView
            report={dupeReport}
            loading={dupesLoading}
            error={dupesError}
            onBack={closeDuplicates}
            onDelete={async (paths) => {
              await deletePaths(paths);
              setDupeReport(null);
            }}
            onReveal={revealInExplorer}
            onOpen={openFile}
          />
        )}
        <StatusBar
          result={scan.result}
          error={scan.error}
          path={currentPath ?? scan.result?.root.path ?? null}
          actionableHint={actionableHint}
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
