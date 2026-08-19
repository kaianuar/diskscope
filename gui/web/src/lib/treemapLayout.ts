// Pure treemap layout: allocates rectangles proportional to node size.
//
// This is the Canvas2D fallback route documented in plan.md Phase 4
// ("fall back to a pure-Canvas2D treemap driven by a `treemap-layout`
// pure function in `gui/web`"). The egui-WASM route is heavier (binary
// size >10 MB, render time, build time); the go/no-go gate in plan.md
// triggers on any of those, so the pure function is the shipped route
// and the sole rendering contract the components exercise.

export type FileTypeName =
  | 'audio'
  | 'video'
  | 'image'
  | 'document'
  | 'code'
  | 'archive'
  | 'directory'
  | 'other';

export interface TreeNode {
  path: string;
  size: number;
  modified: number;
  fileType: FileTypeName;
  children: TreeNode[];
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface LayoutEntry {
  node: TreeNode;
  rect: Rect;
  /** Inclusive depth; the root is depth 0. */
  depth: number;
  /** 0-based index into the output list (stable identity for hover). */
  index: number;
}

export interface LayoutOptions {
  width: number;
  height: number;
}

// Squarify treemap: a deterministic, aspect-ratio-balanced layout that
// allocates area proportional to size. Pure and testable.
//
// Algorithm (Bruls, Huizing, van Wijk 2000):
//   1. Sort entries by size descending.
//   2. Greadily grow a row while adding the next entry does not worsen
//      the worst aspect ratio; when it does, lay the row out along the
//      shorter side, slice it off, and continue with the remainder.
//
// Single-pass: no recursion within emitRow. The remaining rectangle is
// updated in-place after each row is laid out, and the outer loop
// continues into it.
export function squarify(
  entries: TreeNode[],
  rect: Rect,
  depth: number,
  out: LayoutEntry[],
): void {
  if (entries.length === 0 || rect.width <= 0 || rect.height <= 0) return;

  const sorted = [...entries].sort((a, b) => b.size - a.size);
  const total = sorted.reduce((s, n) => s + n.size, 0);
  if (total <= 0) return;

  const area = rect.width * rect.height;
  const scale = area / total;

  // Mutable rectangle that shrinks as rows are laid out.
  let cur: Rect = { ...rect };
  let horizontal = cur.width >= cur.height;

  let row: TreeNode[] = [];
  let rowSum = 0;

  const worst = (r: TreeNode[], sum: number): number => {
    if (r.length === 0 || sum <= 0) return Infinity;
    const length = horizontal ? cur.width : cur.height;
    const thickness = (sum * scale) / length;
    if (thickness <= 0) return Infinity;
    const cellLens = r.map((n) => (n.size * scale) / thickness);
    const minLen = Math.min(...cellLens);
    const maxLen = Math.max(...cellLens);
    return Math.max(length / minLen, thickness / minLen, maxLen / minLen, thickness / maxLen);
  };

  const emitRow = (r: TreeNode[], sum: number): void => {
    if (r.length === 0 || sum <= 0) return;
    const rowArea = sum * scale;
    const length = horizontal ? cur.width : cur.height;
    const thickness = rowArea / length;
    if (thickness <= 0) return;
    let cursor = 0;
    for (const node of r) {
      const cellLen = (node.size * scale) / thickness;
      const cellRect: Rect = horizontal
        ? { x: cur.x + cursor, y: cur.y, width: cellLen, height: thickness }
        : { x: cur.x, y: cur.y + cursor, width: thickness, height: cellLen };
      out.push({ node, rect: cellRect, depth, index: out.length });
      cursor += cellLen;
    }
    // Slice the laid strip off the current rectangle.
    cur = horizontal
      ? { x: cur.x, y: cur.y + thickness, width: cur.width, height: cur.height - thickness }
      : { x: cur.x + thickness, y: cur.y, width: cur.width - thickness, height: cur.height };
    horizontal = cur.width >= cur.height;
  };

  for (const node of sorted) {
    const candidateSum = rowSum + node.size;
    if (row.length === 0 || worst([...row, node], candidateSum) <= worst(row, rowSum)) {
      row.push(node);
      rowSum = candidateSum;
    } else {
      emitRow(row, rowSum);
      row = [node];
      rowSum = node.size;
    }
  }
  if (row.length > 0) emitRow(row, rowSum);
}

/**
 * Compute the treemap layout for a scan root.
 *
 * @param root  the scan result root (a directory node)
 * @param options  target canvas dimensions
 * @returns flat, pre-order list of laid-out entries
 */
export function layoutTreemap(root: TreeNode, options: LayoutOptions): LayoutEntry[] {
  const out: LayoutEntry[] = [];
  if (options.width <= 0 || options.height <= 0) return out;
  const total = root.size > 0 ? root.size : root.children.reduce((s, c) => s + c.size, 0);
  if (total <= 0) return out;
  const rect: Rect = { x: 0, y: 0, width: options.width, height: options.height };
  squarify(root.children, rect, 0, out);
  return out;
}

// FileType → color, mirroring `design-system/tokens.json` colors.
const TYPE_COLORS: Record<FileTypeName, string> = {
  audio: '#2563eb',
  video: '#f59e0b',
  image: '#22c55e',
  document: '#ef4444',
  code: '#e2e8f0',
  archive: '#94a3b8',
  directory: '#1e293b',
  other: '#475569',
};

export function fileTypeColor(t: FileTypeName): string {
  return TYPE_COLORS[t] ?? TYPE_COLORS.other;
}
