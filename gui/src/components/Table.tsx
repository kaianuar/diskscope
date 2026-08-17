import { useState, useMemo, useCallback } from "react";
import type { FileNode } from "../domain";
import { humanSize, humanDate } from "../utils";

export interface TableProps {
  /** Children of the current node to display. */
  nodes: FileNode[];
  /** Currently selected index (-1 = none). */
  selectedIndex: number;
  onSelect: (index: number) => void;
  onActivate: (node: FileNode) => void;
  onContextMenu: (node: FileNode, x: number, y: number) => void;
}

type SortKey = "name" | "size" | "modified" | "node_type";
type SortDir = "asc" | "desc";

/**
 * Table: sortable file list with virtual rendering.
 */
export function Table({
  nodes,
  selectedIndex,
  onSelect,
  onActivate,
  onContextMenu,
}: TableProps) {
  const [sortKey, setSortKey] = useState<SortKey>("size");
  const [sortDir, setSortDir] = useState<SortDir>("desc");

  const sorted = useMemo(() => {
    const copy = [...nodes];
    copy.sort((a, b) => {
      let cmp = 0;
      switch (sortKey) {
        case "name":
          cmp = a.name.localeCompare(b.name);
          break;
        case "size":
          cmp = a.size - b.size;
          break;
        case "modified":
          cmp = new Date(a.modified).getTime() - new Date(b.modified).getTime();
          break;
        case "node_type":
          cmp = a.node_type.localeCompare(b.node_type);
          break;
      }
      return sortDir === "asc" ? cmp : -cmp;
    });
    return copy;
  }, [nodes, sortKey, sortDir]);

  const toggleSort = useCallback(
    (key: SortKey) => {
      if (sortKey === key) {
        setSortDir((d) => (d === "asc" ? "desc" : "asc"));
      } else {
        setSortKey(key);
        setSortDir(key === "name" ? "asc" : "desc");
      }
    },
    [sortKey]
  );

  const sortIndicator = (key: SortKey) =>
    sortKey === key ? (sortDir === "asc" ? " ▲" : " ▼") : "";

  const handleContextMenu = (e: React.MouseEvent, node: FileNode) => {
    e.preventDefault();
    onContextMenu(node, e.clientX, e.clientY);
  };

  if (sorted.length === 0) {
    return (
      <div className="table-empty" data-testid="table-empty">
        No files to display
      </div>
    );
  }

  return (
    <div className="table-container" data-testid="table">
      <table className="file-table">
        <thead>
          <tr>
            <th onClick={() => toggleSort("name")} className="sortable">
              Name{sortIndicator("name")}
            </th>
            <th onClick={() => toggleSort("size")} className="sortable">
              Size{sortIndicator("size")}
            </th>
            <th onClick={() => toggleSort("modified")} className="sortable">
              Modified{sortIndicator("modified")}
            </th>
            <th onClick={() => toggleSort("node_type")} className="sortable">
              Type{sortIndicator("node_type")}
            </th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((node, i) => (
            <tr
              key={node.path}
              className={`table-row ${i === selectedIndex ? "selected" : ""} ${node.is_dir ? "is-dir" : ""}`}
              onClick={() => onSelect(i)}
              onDoubleClick={() => onActivate(node)}
              onContextMenu={(e) => handleContextMenu(e, node)}
              data-testid={`table-row-${node.name}`}
            >
              <td className="cell-name">
                <span className="type-dot" style={{ backgroundColor: typeColor(node.node_type) }} />
                {node.name}
              </td>
              <td className="cell-size">{humanSize(node.size)}</td>
              <td className="cell-modified">{humanDate(node.modified)}</td>
              <td className="cell-type">{node.node_type}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function typeColor(type: string): string {
  const colors: Record<string, string> = {
    Image: "#4A90D9",
    Video: "#E74C3C",
    Audio: "#9B59B6",
    Document: "#F39C12",
    Code: "#2ECC71",
    Archive: "#1ABC9C",
    Other: "#95A5A6",
  };
  return colors[type] ?? colors.Other;
}
