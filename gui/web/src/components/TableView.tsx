// Sortable table of the current directory's entries.

import { useMemo } from 'react';
import { formatSize } from '../lib/formatSize';
import type { FileNode, SortColumn, SortDirection } from '../ipc';

export interface TableViewProps {
  entries: FileNode[];
  /** Column to sort by (null = natural order). */
  sortColumn: SortColumn | null;
  sortDirection: SortDirection;
  onSort: (column: SortColumn) => void;
  onActivate: (entry: FileNode) => void;
}

function typeLabel(t: FileNode['fileType']): string {
  return t.charAt(0).toUpperCase() + t.slice(1);
}

export function TableView({ entries, sortColumn, sortDirection, onSort, onActivate }: TableViewProps) {
  const sorted = useMemo(() => {
    if (!sortColumn) return entries;
    const dir = sortDirection === 'asc' ? 1 : -1;
    return [...entries].sort((a, b) => {
      switch (sortColumn) {
        case 'name':
          return a.path.localeCompare(b.path) * dir;
        case 'size':
          return (a.size - b.size) * dir;
        case 'modified':
          return (a.modified - b.modified) * dir;
        case 'type':
          return typeLabel(a.fileType).localeCompare(typeLabel(b.fileType)) * dir;
      }
    });
  }, [entries, sortColumn, sortDirection]);

  const header = (label: string, column: SortColumn): JSX.Element => (
    <th
      onClick={() => onSort(column)}
      role="columnheader"
      aria-sort={sortColumn === column ? (sortDirection === 'asc' ? 'ascending' : 'descending') : 'none'}
    >
      {label}
      {sortColumn === column ? (sortDirection === 'asc' ? ' ▲' : ' ▼') : ''}
    </th>
  );

  return (
    <table data-testid="file-table">
      <thead>
        <tr>
          {header('Name', 'name')}
          {header('Size', 'size')}
          {header('Modified', 'modified')}
          {header('Type', 'type')}
        </tr>
      </thead>
      <tbody>
        {sorted.map((entry) => (
          <tr
            key={entry.path}
            data-testid="table-row"
            onDoubleClick={() => onActivate(entry)}
          >
            <td>{entry.path.split('/').filter(Boolean).pop() ?? entry.path}</td>
            <td>{formatSize(entry.size)}</td>
            <td>{new Date(entry.modified * 1000).toLocaleString()}</td>
            <td>{typeLabel(entry.fileType)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
