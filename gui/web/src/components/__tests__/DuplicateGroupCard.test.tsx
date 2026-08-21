import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DuplicateGroupCard } from '../DuplicateGroupCard';
import type { DuplicateGroup } from '../../ipc';

const group: DuplicateGroup = {
  hash: 'abc123',
  size: 1024,
  files: ['/keep.txt', '/dup1.txt', '/dup2.txt'],
};

describe('DuplicateGroupCard', () => {
  it('should render group header with recoverable size and file count', () => {
    render(
      <DuplicateGroupCard
        group={group}
        onDelete={() => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getByTestId('dupe-group-header')).toHaveTextContent('2.0 KiB recoverable');
    expect(screen.getByTestId('dupe-group-header')).toHaveTextContent('3 files');
  });

  it('should render all file rows when expanded (default)', () => {
    render(
      <DuplicateGroupCard
        group={group}
        onDelete={() => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getAllByTestId(/^dupe-row-/)).toHaveLength(3);
  });

  it('should collapse rows when header clicked, expand on second click', () => {
    render(
      <DuplicateGroupCard
        group={group}
        onDelete={() => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getAllByTestId(/^dupe-row-/)).toHaveLength(3);
    fireEvent.click(screen.getByTestId('dupe-group-header'));
    expect(screen.queryAllByTestId(/^dupe-row-/)).toHaveLength(0);
    fireEvent.click(screen.getByTestId('dupe-group-header'));
    expect(screen.getAllByTestId(/^dupe-row-/)).toHaveLength(3);
  });

  it('should mark first file as keeper with disabled checkbox', () => {
    render(
      <DuplicateGroupCard
        group={group}
        onDelete={() => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getByTestId('dupe-keeper-0')).toHaveTextContent('keep');
    // Keeper row should not have a checkbox.
    expect(screen.queryByTestId('dupe-check-0')).toBeNull();
  });

  it('should have checkboxes for non-keeper files, default checked', () => {
    render(
      <DuplicateGroupCard
        group={group}
        onDelete={() => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    const check1 = screen.getByTestId('dupe-check-1') as HTMLInputElement;
    const check2 = screen.getByTestId('dupe-check-2') as HTMLInputElement;
    expect(check1.checked).toBe(true);
    expect(check2.checked).toBe(true);
  });

  it('should call onDelete with checked paths minus keeper when delete clicked', () => {
    const onDelete = vi.fn();
    render(
      <DuplicateGroupCard
        group={group}
        onDelete={onDelete}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    fireEvent.click(screen.getByTestId('dupe-delete'));
    expect(onDelete).toHaveBeenCalledWith(['/dup1.txt', '/dup2.txt']);
  });

  it('should exclude unchecked files from delete', () => {
    const onDelete = vi.fn();
    render(
      <DuplicateGroupCard
        group={group}
        onDelete={onDelete}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    // Uncheck second file.
    fireEvent.click(screen.getByTestId('dupe-check-1'));
    fireEvent.click(screen.getByTestId('dupe-delete'));
    expect(onDelete).toHaveBeenCalledWith(['/dup2.txt']);
  });

  it('should call onOpen on double-click', () => {
    const onOpen = vi.fn();
    render(
      <DuplicateGroupCard
        group={group}
        onDelete={() => undefined}
        onReveal={() => undefined}
        onOpen={onOpen}
      />,
    );
    fireEvent.doubleClick(screen.getByTestId('dupe-row-1'));
    expect(onOpen).toHaveBeenCalledWith('/dup1.txt');
  });
});
