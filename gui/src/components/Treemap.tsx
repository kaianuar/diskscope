import { useMemo, useState } from "react";
import type { FileNode } from "../domain";
import { humanSize } from "../utils";

export interface TreemapProps {
  node: FileNode;
  onDrill: (node: FileNode) => void;
  /** Total parent size for percentage calculation. */
  parentSize: number;
}

interface TreemapRect {
  node: FileNode;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Squarified treemap layout algorithm. */
function squarify(
  items: FileNode[],
  x: number,
  y: number,
  w: number,
  h: number
): TreemapRect[] {
  if (items.length === 0 || w <= 0 || h <= 0) return [];

  const total = items.reduce((s, n) => s + n.size, 0);
  if (total === 0) return [];

  const rects: TreemapRect[] = [];
  let remaining = [...items];
  let cx = x;
  let cy = y;
  let rw = w;
  let rh = h;

  while (remaining.length > 0) {
    const remainingSize = remaining.reduce((s, n) => s + n.size, 0);
    const horizontal = rw >= rh;

    // Lay out items along the shorter axis
    const row: FileNode[] = [];
    let rowSize = 0;
    const shortSide = Math.min(rw, rh);

    for (let i = 0; i < remaining.length; i++) {
      const candidate = [...row, remaining[i]];
      const candidateSize = rowSize + remaining[i].size;
      if (row.length > 0 && worstAspectRatio(candidate, candidateSize, shortSide) > worstAspectRatio(row, rowSize, shortSide)) {
        break;
      }
      row.push(remaining[i]);
      rowSize = candidateSize;
    }

    const rowFraction = rowSize / remainingSize;

    if (horizontal) {
      const rowWidth = rw * rowFraction;
      let oy = cy;
      for (const item of row) {
        const itemHeight = (item.size / rowSize) * rh;
        rects.push({ node: item, x: cx, y: oy, w: rowWidth, h: itemHeight });
        oy += itemHeight;
      }
      cx += rowWidth;
      rw -= rowWidth;
    } else {
      const rowHeight = rh * rowFraction;
      let ox = cx;
      for (const item of row) {
        const itemWidth = (item.size / rowSize) * rw;
        rects.push({ node: item, x: ox, y: cy, w: itemWidth, h: rowHeight });
        ox += itemWidth;
      }
      cy += rowHeight;
      rh -= rowHeight;
    }

    remaining = remaining.slice(row.length);
  }

  return rects;
}

function worstAspectRatio(row: FileNode[], rowSize: number, shortSide: number): number {
  if (rowSize === 0 || shortSide === 0) return Infinity;
  const max = Math.max(...row.map((n) => n.size));
  const min = Math.min(...row.map((n) => n.size));
  const sideSquared = shortSide * shortSide;
  const rowSizeSquared = rowSize * rowSize;
  return Math.max(
    (sideSquared * max) / rowSizeSquared,
    rowSizeSquared / (sideSquared * min)
  );
}

/** Color map for file types. */
const TYPE_COLORS: Record<string, string> = {
  Image: "#4A90D9",
  Video: "#E74C3C",
  Audio: "#9B59B6",
  Document: "#F39C12",
  Code: "#2ECC71",
  Archive: "#1ABC9C",
  Other: "#95A5A6",
};

const TYPE_COLORS_LIGHT: Record<string, string> = {
  Image: "#D6E8F7",
  Video: "#FADBD8",
  Audio: "#E8DAEF",
  Document: "#FDEBD0",
  Code: "#D5F5E3",
  Archive: "#D1F2EB",
  Other: "#EAECEE",
};

/**
 * Treemap: interactive canvas treemap visualization of file tree.
 * Uses HTML divs with absolute positioning (no Canvas API needed).
 */
export function Treemap({ node, onDrill, parentSize }: TreemapProps) {
  const [hovered, setHovered] = useState<FileNode | null>(null);
  const [tooltipPos, setTooltipPos] = useState({ x: 0, y: 0 });

  const sortedChildren = useMemo(
    () => [...node.children].sort((a, b) => b.size - a.size),
    [node.children]
  );

  // Only show top 50 children to avoid visual clutter
  const displayChildren = sortedChildren.slice(0, 50);
  const rects = squarify(displayChildren, 0, 0, 100, 100);

  return (
    <div className="treemap-container" data-testid="treemap">
      <div className="treemap" style={{ position: "relative", width: "100%", height: "100%" }}>
        {rects.map((r) => {
          const pct = parentSize > 0 ? ((r.node.size / parentSize) * 100).toFixed(1) : "0";
          const isSmall = r.w < 5 || r.h < 5;
          return (
            <div
              key={r.node.path}
              className="treemap-cell"
              data-testid={`treemap-cell-${r.node.name}`}
              style={{
                position: "absolute",
                left: `${r.x}%`,
                top: `${r.y}%`,
                width: `${r.w}%`,
                height: `${r.h}%`,
                backgroundColor: r.node.is_dir
                  ? TYPE_COLORS_LIGHT[r.node.node_type] ?? TYPE_COLORS_LIGHT.Other
                  : TYPE_COLORS[r.node.node_type] ?? TYPE_COLORS.Other,
                border: "1px solid rgba(255,255,255,0.3)",
                overflow: "hidden",
                cursor: r.node.is_dir ? "pointer" : "default",
                boxSizing: "border-box",
              }}
              onClick={() => r.node.is_dir && onDrill(r.node)}
              onMouseEnter={(e) => {
                setHovered(r.node);
                setTooltipPos({ x: e.clientX, y: e.clientY });
              }}
              onMouseMove={(e) => {
                setTooltipPos({ x: e.clientX, y: e.clientY });
              }}
              onMouseLeave={() => setHovered(null)}
            >
              {!isSmall && (
                <div className="treemap-label">
                  <span className="treemap-name">{r.node.name}</span>
                  <span className="treemap-size">{pct}%</span>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {hovered && (
        <div
          className="treemap-tooltip"
          data-testid="treemap-tooltip"
          style={{
            position: "fixed",
            left: tooltipPos.x + 12,
            top: tooltipPos.y + 12,
            pointerEvents: "none",
          }}
        >
          <strong>{hovered.name}</strong>
          <br />
          {humanSize(hovered.size)} (
          {parentSize > 0 ? ((hovered.size / parentSize) * 100).toFixed(1) : "0"}%)
          {hovered.is_dir && <><br />📁 Directory</>}
        </div>
      )}
    </div>
  );
}
