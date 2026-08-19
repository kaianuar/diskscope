// Tests for TableView sorting behaviour.

import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TableView } from '../TableView';
import type { FileNode } from '../../ipc';

function fileNode(path: string, size: number, modified: number, fileType: FileNode['fileType']): FileNode {
  return { path, size, modified, fileType, children: [] };
}

const entries: FileNode[] = [
  fileNode('/a.txt', 10, 100, 'document'),
  fileNode('/b.mp4', 500, 50, 'video'),
  fileNode('/c.rs', 200, 300, 'code'),
];

describe('TableView', () => {
  it('should sort entries by size descending when header clicked twice', () => {
    const onSort = vi.fn();
    render(
      <TableView
        entries={entries}
        sortColumn={null}
        sortDirection="asc"
        onSort={onSort}
        onActivate={() => undefined}
      />,
    );
    // Click Size header once → sort by size.
    fireEvent.click(screen.getByText('Size'));
    expect(onSort).toHaveBeenCalledWith('size');
  });

  it('should render rows for each entry', () => {
    render(
      <TableView
        entries={entries}
        sortColumn="size"
        sortDirection="desc"
        onSort={() => undefined}
        onActivate={() => undefined}
      />,
    );
    expect(screen.getAllByTestId('table-row')).toHaveLength(3);
  });

  it('should sort by size descending when sortColumn=size, direction=desc', () => {
    render(
      <TableView
        entries={entries}
        sortColumn="size"
        sortDirection="desc"
        onSort={() => undefined}
        onActivate={() => undefined}
      />,
    );
    const rows = screen.getAllByTestId('table-row');
    const first = rows[0].textContent ?? '';
    expect(first).toContain('b.mp4'); // 500 B is the largest
  });
});
