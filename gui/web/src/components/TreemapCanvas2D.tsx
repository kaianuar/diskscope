// Canvas2D treemap renderer — the shipped Phase 4 route (see
// `gui/web/src/lib/treemapLayout.ts` for the go/no-go rationale).
//
// Interaction contract:
// - Single-click on a directory block → `onActivate` (navigate into it).
// - Double-click on a leaf block → `onOpen` (open the file with the OS
//   default app). A double-click on a directory also resolves to
//   `onActivate` (navigate).
// - `actionableIndex` (when set) marks the block under the pointer as
//   actionable so callers can show affordance (cursor / hint).

import { useEffect, useRef } from 'react';
import type { FileNode } from '../ipc';
import { fileTypeColor, layoutTreemap, type LayoutEntry } from '../lib/treemapLayout';

export interface TreemapCanvas2DProps {
  root: FileNode;
  /** Index of the hovered entry, or null. */
  hoveredIndex: number | null;
  /** Index of the actionable entry under the pointer, or null. */
  actionableIndex: number | null;
  onHover: (index: number | null) => void;
  /** Single-click on a directory (navigate). */
  onActivate: (entry: LayoutEntry) => void;
  /** Double-click on a leaf (open with the OS default app). */
  onOpen: (entry: LayoutEntry) => void;
}

// Browser delay between the two clicks of a double-click (used to avoid
// firing a single-click navigation on the first click of a double-click).
const DOUBLE_CLICK_MS = 300;

/** Hit-test a mouse event against the laid-out entries. */
function hitTest(
  e: MouseEvent,
  entries: LayoutEntry[],
  canvas: HTMLCanvasElement,
): LayoutEntry | null {
  const bounds = canvas.getBoundingClientRect();
  const x = e.clientX - bounds.left;
  const y = e.clientY - bounds.top;
  return (
    entries.find(
      (en) =>
        x >= en.rect.x && x < en.rect.x + en.rect.width && y >= en.rect.y && y < en.rect.y + en.rect.height,
    ) ?? null
  );
}

export function TreemapCanvas2D({
  root,
  hoveredIndex,
  actionableIndex,
  onHover,
  onActivate,
  onOpen,
}: TreemapCanvas2DProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const lastClick = useRef<{ time: number; index: number } | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const entries = layoutTreemap(root, { width: canvas.width, height: canvas.height });

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    for (const entry of entries) {
      const { rect, node } = entry;
      if (rect.width < 1 || rect.height < 1) continue;
      ctx.fillStyle = fileTypeColor(node.fileType);
      ctx.fillRect(rect.x, rect.y, rect.width, rect.height);
      if (hoveredIndex === entry.index) {
        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = 2;
        ctx.strokeRect(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2);
      }
    }

    const handleMove = (e: MouseEvent): void => {
      const found = hitTest(e, entries, canvas);
      onHover(found ? found.index : null);
    };
    const handleClick = (e: MouseEvent): void => {
      const found = hitTest(e, entries, canvas);
      if (!found) return;
      const now = Date.now();
      const prev = lastClick.current;
      lastClick.current = { time: now, index: found.index };
      // Second click of a double-click: open leaves, navigate directories.
      if (prev && prev.index === found.index && now - prev.time <= DOUBLE_CLICK_MS) {
        if (found.node.fileType === 'directory') {
          onActivate(found);
        } else {
          onOpen(found);
        }
        lastClick.current = null; // swallow the pending single-click action
        return;
      }
      // First click of a (possible) double-click: navigate directories
      // immediately; leaves wait to see if a second click follows.
      if (found.node.fileType === 'directory') {
        onActivate(found);
      }
    };
    canvas.addEventListener('mousemove', handleMove);
    canvas.addEventListener('click', handleClick);
    return () => {
      canvas.removeEventListener('mousemove', handleMove);
      canvas.removeEventListener('click', handleClick);
    };
  }, [root, hoveredIndex, onHover, onActivate, onOpen]);

  const actionable = actionableIndex !== null;
  return (
    <canvas
      ref={canvasRef}
      data-testid="treemap-canvas"
      className="treemap-canvas"
      width={800}
      height={600}
      style={{ cursor: actionable ? 'pointer' : 'default' }}
    />
  );
}
