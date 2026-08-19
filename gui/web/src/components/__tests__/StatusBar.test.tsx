// Tests for StatusBar: summary, error, and the actionable hint.

import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { StatusBar } from '../StatusBar';
import type { ScanResult } from '../../ipc';

const result: ScanResult = {
  root: { path: '/', size: 100, modified: 0, fileType: 'directory', children: [] },
  totalSize: 100,
  fileCount: 2,
  scanDurationMs: 5,
  skipped: [],
};

describe('StatusBar', () => {
  it('should show "No scan yet" when no result and no error', () => {
    render(<StatusBar result={null} error={null} path={null} actionableHint={null} />);
    expect(screen.getByTestId('status-path')).toHaveTextContent('No scan yet');
    expect(screen.queryByTestId('status-hint')).toBeNull();
  });

  it('should show the scan summary when a result exists', () => {
    render(<StatusBar result={result} error={null} path="/" actionableHint={null} />);
    expect(screen.getByTestId('status-summary')).toHaveTextContent('entries');
  });

  it('should show the error instead of summary when an error is present', () => {
    render(<StatusBar result={result} error="boom" path="/" actionableHint="Click to open file" />);
    expect(screen.getByTestId('scan-error')).toHaveTextContent('boom');
    expect(screen.queryByTestId('status-summary')).toBeNull();
    expect(screen.queryByTestId('status-hint')).toBeNull();
  });

  it('should show the actionable hint when provided', () => {
    render(<StatusBar result={result} error={null} path="/" actionableHint="Click to open file" />);
    expect(screen.getByTestId('status-hint')).toHaveTextContent('Click to open file');
  });

  it('should not show the hint when nothing is actionable', () => {
    render(<StatusBar result={result} error={null} path="/" actionableHint={null} />);
    expect(screen.queryByTestId('status-hint')).toBeNull();
  });
});
