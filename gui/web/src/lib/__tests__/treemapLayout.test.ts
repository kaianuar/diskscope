// Tests for the pure treemap layout + FileType coloring.

import { describe, expect, it } from 'vitest';
import { fileTypeColor, layoutTreemap, type TreeNode } from '../../lib/treemapLayout';

function node(path: string, size: number, fileType: TreeNode['fileType'] = 'other'): TreeNode {
  return { path, size, modified: 0, fileType, children: [] };
}

describe('treemap layout', () => {
  it('should allocate area proportional to size when layout called with ScanResult', () => {
    const root: TreeNode = {
      path: '/',
      size: 100,
      modified: 0,
      fileType: 'directory',
      children: [node('/a', 60), node('/b', 30), node('/c', 10)],
    };
    const entries = layoutTreemap(root, { width: 100, height: 100 });
    expect(entries).toHaveLength(3);
    // Total allocated area equals the canvas area.
    const totalArea = entries.reduce((s, e) => s + e.rect.width * e.rect.height, 0);
    expect(totalArea).toBeCloseTo(10000, 0);
    // Bigger files get bigger cells.
    const a = entries.find((e) => e.node.path === '/a');
    const c = entries.find((e) => e.node.path === '/c');
    expect(a).toBeDefined();
    expect(c).toBeDefined();
    expect(a!.rect.width * a!.rect.height).toBeGreaterThan(c!.rect.width * c!.rect.height);
  });

  it('should color by FileType when layout called', () => {
    const root: TreeNode = {
      path: '/',
      size: 100,
      modified: 0,
      fileType: 'directory',
      children: [node('/v.mp4', 100, 'video')],
    };
    const entries = layoutTreemap(root, { width: 100, height: 100 });
    expect(entries).toHaveLength(1);
    // Every laid-out entry maps to a color by its file type.
    for (const e of entries) {
      expect(fileTypeColor(e.node.fileType)).toMatch(/^#[0-9a-f]{6}$/i);
    }
    expect(fileTypeColor('video')).toBe('#f59e0b');
  });

  it('should reveal hovered entry when layout called with hover index', () => {
    const root: TreeNode = {
      path: '/',
      size: 100,
      modified: 0,
      fileType: 'directory',
      children: [node('/a', 50), node('/b', 50)],
    };
    const entries = layoutTreemap(root, { width: 100, height: 100 });
    // Stable 0-based indices across the output.
    expect(entries.map((e) => e.index)).toEqual([0, 1]);
    const hovered = entries.find((e) => e.index === 1);
    expect(hovered?.node.path).toBe('/b');
    // The hovered entry's rect is within the canvas.
    expect(hovered!.rect.x + hovered!.rect.width).toBeLessThanOrEqual(100);
    expect(hovered!.rect.y + hovered!.rect.height).toBeLessThanOrEqual(100);
  });

  it('should return empty layout for an empty root', () => {
    const root: TreeNode = { path: '/', size: 0, modified: 0, fileType: 'directory', children: [] };
    expect(layoutTreemap(root, { width: 100, height: 100 })).toEqual([]);
  });
});
