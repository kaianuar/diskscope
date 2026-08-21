import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DuplicatesView } from '../DuplicatesView';
import type { DuplicateReport } from '../../ipc';

const report: DuplicateReport = {
  groups: [
    { hash: 'aaa', size: 1024, files: ['/a.txt', '/b.txt'] },
    { hash: 'bbb', size: 2048, files: ['/c.bin', '/d.bin', '/e.bin'] },
  ],
  totalRecoverable: 3072,
  totalDuplicateFiles: 3,
};

describe('DuplicatesView', () => {
  it('should render summary from report', () => {
    render(
      <DuplicatesView
        report={report}
        loading={false}
        error={null}
        onBack={() => undefined}
        onDelete={async () => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getByTestId('dupes-summary')).toHaveTextContent('2 duplicate groups');
    expect(screen.getByTestId('dupes-summary')).toHaveTextContent('3.0 KiB recoverable');
  });

  it('should render group cards for each group', () => {
    render(
      <DuplicatesView
        report={report}
        loading={false}
        error={null}
        onBack={() => undefined}
        onDelete={async () => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getAllByTestId('dupe-group')).toHaveLength(2);
  });

  it('should show loading state', () => {
    render(
      <DuplicatesView
        report={null}
        loading={true}
        error={null}
        onBack={() => undefined}
        onDelete={async () => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getByTestId('dupes-loading')).toHaveTextContent('Scanning for duplicates');
  });

  it('should show empty state when report has no groups', () => {
    render(
      <DuplicatesView
        report={{ groups: [], totalRecoverable: 0, totalDuplicateFiles: 0 }}
        loading={false}
        error={null}
        onBack={() => undefined}
        onDelete={async () => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getByTestId('dupes-empty')).toHaveTextContent('No duplicate files found');
  });

  it('should show error state', () => {
    render(
      <DuplicatesView
        report={null}
        loading={false}
        error="something broke"
        onBack={() => undefined}
        onDelete={async () => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getByTestId('dupes-error')).toHaveTextContent('something broke');
  });

  it('should call onBack when back button clicked', () => {
    const onBack = vi.fn();
    render(
      <DuplicatesView
        report={report}
        loading={false}
        error={null}
        onBack={onBack}
        onDelete={async () => undefined}
        onReveal={() => undefined}
        onOpen={() => undefined}
      />,
    );
    fireEvent.click(screen.getByTestId('dupes-back'));
    expect(onBack).toHaveBeenCalled();
  });
});
