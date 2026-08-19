// Tests for TreemapCanvas2D click interactions:
// single-click on a directory navigates; double-click on a leaf opens.

import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render } from '@testing-library/react';
import { TreemapCanvas2D } from '../TreemapCanvas2D';
import type { FileNode } from '../../ipc';

function fileNode(path: string, size: number, fileType: FileNode['fileType']): FileNode {
  return { path, size, modified: 0, fileType, children: [] };
}

// jsdom does not implement canvas 2D; stub it so the component's effect
// can attach its event listeners.
const fakeContext = {
  clearRect: vi.fn(),
  fillRect: vi.fn(),
  strokeRect: vi.fn(),
  set fillStyle(_v: string) {},
  set strokeStyle(_v: string) {},
  set lineWidth(_v: number) {},
};
HTMLCanvasElement.prototype.getContext = vi.fn(() => fakeContext) as never;

// Two children with big enough areas to be hit-testable. squarify lays
// equal-size children out as stacked rows: the directory is the top half
// (y 0–300), the leaf the bottom half (y 300–600) of the 800x600 canvas.
const root: FileNode = {
  path: '/',
  size: 400,
  modified: 0,
  fileType: 'directory',
  children: [
    fileNode('/dir', 200, 'directory'),
    fileNode('/leaf.txt', 200, 'document'),
  ],
};

const DIR_X = 400;
const DIR_Y = 150;
const LEAF_X = 400;
const LEAF_Y = 450;

function renderTreemap(overrides: Partial<Parameters<typeof TreemapCanvas2D>[0]> = {}) {
  const onActivate = vi.fn();
  const onOpen = vi.fn();
  const onHover = vi.fn();
  render(
    <TreemapCanvas2D
      root={root}
      hoveredIndex={null}
      actionableIndex={null}
      onHover={onHover}
      onActivate={onActivate}
      onOpen={onOpen}
      {...overrides}
    />,
  );
  return { onActivate, onOpen, onHover };
}

describe('TreemapCanvas2D', () => {
  it('should navigate when a directory block is single-clicked', () => {
    const { onActivate, onOpen } = renderTreemap();
    const canvas = document.querySelector('[data-testid="treemap-canvas"]') as HTMLCanvasElement;
    // Left half of the 800x600 canvas = the directory block.
    fireEvent.click(canvas, { clientX: DIR_X, clientY: DIR_Y });
    expect(onActivate).toHaveBeenCalledTimes(1);
    expect(onActivate.mock.calls[0][0].node.path).toBe('/dir');
    expect(onOpen).not.toHaveBeenCalled();
  });

  it('should not open when a leaf block is single-clicked', () => {
    const { onActivate, onOpen } = renderTreemap();
    const canvas = document.querySelector('[data-testid="treemap-canvas"]') as HTMLCanvasElement;
    // Right half of the canvas = the leaf block.
    fireEvent.click(canvas, { clientX: LEAF_X, clientY: LEAF_Y });
    expect(onOpen).not.toHaveBeenCalled();
    expect(onActivate).not.toHaveBeenCalled();
  });

  it('should open when a leaf block is double-clicked', () => {
    vi.useFakeTimers();
    const { onActivate, onOpen } = renderTreemap();
    const canvas = document.querySelector('[data-testid="treemap-canvas"]') as HTMLCanvasElement;
    // Double-click the leaf block (right half).
    fireEvent.click(canvas, { clientX: LEAF_X, clientY: LEAF_Y });
    act(() => {
      vi.advanceTimersByTime(50);
    });
    fireEvent.click(canvas, { clientX: LEAF_X, clientY: LEAF_Y });
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen.mock.calls[0][0].node.path).toBe('/leaf.txt');
    expect(onActivate).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it('should navigate when a directory block is double-clicked', () => {
    vi.useFakeTimers();
    const { onActivate, onOpen } = renderTreemap();
    const canvas = document.querySelector('[data-testid="treemap-canvas"]') as HTMLCanvasElement;
    fireEvent.click(canvas, { clientX: DIR_X, clientY: DIR_Y });
    act(() => {
      vi.advanceTimersByTime(50);
    });
    fireEvent.click(canvas, { clientX: DIR_X, clientY: DIR_Y });
    // Directory double-click navigates (twice: first-click + double-click).
    expect(onActivate).toHaveBeenCalledTimes(2);
    expect(onOpen).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});
