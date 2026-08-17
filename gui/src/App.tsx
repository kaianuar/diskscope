import { useState, useCallback, useRef } from "react";
import { Sidebar } from "./components/Sidebar";
import { Toolbar } from "./components/Toolbar";
import { Treemap } from "./components/Treemap";
import { Table } from "./components/Table";
import { FilterBar } from "./components/FilterBar";
import { ContextMenu } from "./components/ContextMenu";
import { useScan, useKeyboard } from "./hooks";
import { deleteFile, undoDelete } from "./ipc";
import type { ScanFilter, FileNode } from "./domain";
import "./App.css";

type ViewMode = "treemap" | "table";

/**
 * Main application layout: sidebar + toolbar + content area.
 */
export default function App() {
  const scan = useScan("/");
  const [filter, setFilter] = useState<ScanFilter>({});
  const [viewMode, setViewMode] = useState<ViewMode>("table");
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [contextMenu, setContextMenu] = useState<{
    node: FileNode;
    x: number;
    y: number;
  } | null>(null);
  const filterRef = useRef<HTMLInputElement>(null);

  const children = scan.currentNode?.children ?? [];
  const parentSize = scan.currentNode?.size ?? 0;

  const handleScan = useCallback(
    (path: string) => {
      scan.scan(path, filter);
    },
    [scan.scan, filter]
  );

  const handleRefresh = useCallback(() => {
    if (scan.result) scan.scan(scan.result.root, filter);
  }, [scan.scan, scan.result, filter]);

  const handleDelete = useCallback(
    async (node?: FileNode) => {
      const target = node ?? children[selectedIndex];
      if (!target) return;
      try {
        await deleteFile(target.path);
        handleRefresh();
      } catch (e) {
        console.error("Delete failed:", e);
      }
    },
    [children, selectedIndex, handleRefresh]
  );

  const handleUndo = useCallback(async () => {
    try {
      await undoDelete();
      handleRefresh();
    } catch (e) {
      console.error("Undo failed:", e);
    }
  }, [handleRefresh]);

  const handleActivate = useCallback(
    (node: FileNode) => {
      if (node.is_dir) {
        scan.drillInto(node);
        setSelectedIndex(-1);
      }
    },
    [scan.drillInto]
  );

  const handleContextMenu = useCallback(
    (node: FileNode, x: number, y: number) => {
      setContextMenu({ node, x, y });
    },
    []
  );

  const copyToClipboard = useCallback((text: string) => {
    navigator.clipboard?.writeText(text).catch(() => {});
  }, []);

  // Keyboard navigation
  useKeyboard({
    onArrowUp: () => setSelectedIndex((i) => Math.max(0, i - 1)),
    onArrowDown: () =>
      setSelectedIndex((i) => Math.min(children.length - 1, i + 1)),
    onArrowLeft: () => scan.goUp(),
    onEnter: () => {
      if (children[selectedIndex]) handleActivate(children[selectedIndex]);
    },
    onBackspace: () => scan.goUp(),
    onDelete: () => handleDelete(),
    onUndo: handleUndo,
    onSearch: () => filterRef.current?.focus(),
  });

  const breadcrumbs = [
    ...(scan.breadcrumbs.map((n) => n.name).reverse() || []),
    scan.currentNode?.name || "",
  ];

  return (
    <div className="app" data-testid="app">
      <Sidebar
        onScan={handleScan}
        result={scan.currentNode}
        totalSize={parentSize}
      />

      <main className="main-panel">
        <Toolbar
          canGoUp={scan.breadcrumbs.length > 0}
          loading={scan.loading}
          onGoUp={() => {
            scan.goUp();
            setSelectedIndex(-1);
          }}
          onRefresh={handleRefresh}
          breadcrumbs={breadcrumbs}
          onBreadcrumbClick={(i) => {
            scan.goToBreadcrumb(breadcrumbs.length - 1 - i);
            setSelectedIndex(-1);
          }}
        />

        <div className="view-toggle">
          <button
            className={viewMode === "table" ? "active" : ""}
            onClick={() => setViewMode("table")}
          >
            Table
          </button>
          <button
            className={viewMode === "treemap" ? "active" : ""}
            onClick={() => setViewMode("treemap")}
          >
            Treemap
          </button>
        </div>

        <FilterBar filter={filter} onChange={setFilter} />

        {scan.error && (
          <div className="error-banner" data-testid="error">
            {scan.error}
          </div>
        )}

        {scan.loading && (
          <div className="loading" data-testid="loading">
            Scanning…
          </div>
        )}

        <div className="content-area">
          {scan.currentNode && viewMode === "treemap" && (
            <Treemap
              node={scan.currentNode}
              onDrill={handleActivate}
              parentSize={parentSize}
            />
          )}

          {viewMode === "table" && (
            <Table
              nodes={children}
              selectedIndex={selectedIndex}
              onSelect={setSelectedIndex}
              onActivate={handleActivate}
              onContextMenu={handleContextMenu}
            />
          )}
        </div>
      </main>

      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          visible
          onOpenExplorer={() => {
            // Tauri shell.open would go here
            setContextMenu(null);
          }}
          onCopyPath={() => {
            copyToClipboard(contextMenu.node.path);
            setContextMenu(null);
          }}
          onCopySize={() => {
            copyToClipboard(String(contextMenu.node.size));
            setContextMenu(null);
          }}
          onDelete={() => {
            handleDelete(contextMenu.node);
            setContextMenu(null);
          }}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}
