// Canvas2D treemap renderer — the shipped Phase 4 route (see
// `gui/web/src/lib/treemapLayout.ts` for the go/no-go rationale).

import { useEffect, useRef } from 'react';
import type { FileNode } from '../ipc';
import { fileTypeColor, layoutTreemap, type LayoutEntry } from '../lib/treemapLayout';

export interface TreemapCanvas2DProps {
  root: FileNode;
  /** Index of the hovered entry, or null. */
  hoveredIndex: number | null;
  onHover: (index: number | null) => void;
  onActivate: (entry: LayoutEntry) => void;
}

export function TreemapCanvas2D({ root, hoveredIndex, onHover, onActivate }: TreemapCanvas2DProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

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
      const bounds = canvas.getBoundingClientRect();
      const x = e.clientX - bounds.left;
      const y = e.clientY - bounds.top;
      const found = entries.find(
        (en) =>
          x >= en.rect.x && x < en.rect.x + en.rect.width && y >= en.rect.y && y < en.rect.y + en.rect.height,
      );
      onHover(found ? found.index : null);
    };
    const handleClick = (e: MouseEvent): void => {
      const bounds = canvas.getBoundingClientRect();
      const x = e.clientX - bounds.left;
      const y = e.clientY - bounds.top;
      const found = entries.find(
        (en) =>
          x >= en.rect.x && x < en.rect.x + en.rect.width && y >= en.rect.y && y < en.rect.y + en.rect.height,
      );
      if (found && found.node.fileType === 'directory') onActivate(found);
    };
    canvas.addEventListener('mousemove', handleMove);
    canvas.addEventListener('click', handleClick);
    return () => {
      canvas.removeEventListener('mousemove', handleMove);
      canvas.removeEventListener('click', handleClick);
    };
  }, [root, hoveredIndex, onHover, onActivate]);

  return (
    <canvas
      ref={canvasRef}
      data-testid="treemap-canvas"
      className="treemap-canvas"
      width={800}
      height={600}
    />
  );
}
