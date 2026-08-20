// Tests for App theme persistence.

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, act } from '@testing-library/react';
import { App } from '../App';

// Mock Tauri APIs — App renders useScan which calls listen().
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('../../ipc', () => ({
  scan: vi.fn(() => Promise.resolve({ root: { path: '/', size: 0, modified: 0, fileType: 'directory', children: [] }, totalSize: 0, fileCount: 0, scanDurationMs: 0, skipped: [] })),
  cancelScan: vi.fn(() => Promise.resolve()),
  deletePaths: vi.fn(() => Promise.resolve()),
  undoLastDelete: vi.fn(() => Promise.resolve()),
  openFile: vi.fn(() => Promise.resolve()),
}));

describe('App theme', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
  });

  it('should default to dark theme when no saved preference', () => {
    act(() => {
      render(<App />);
    });
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('should set data-theme to light when localStorage has "light"', () => {
    localStorage.setItem('diskscope-theme', 'light');
    act(() => {
      render(<App />);
    });
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });
});
