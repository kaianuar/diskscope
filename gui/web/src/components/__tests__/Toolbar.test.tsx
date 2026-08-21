// Tests for Toolbar theme toggle.

import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Toolbar } from '../Toolbar';

const defaultProps = {
  scanning: false,
  progress: null,
  onCancel: vi.fn(),
  onRescan: vi.fn(),
  canGoUp: false,
  onGoUp: vi.fn(),
  canGoBack: false,
  onGoBack: vi.fn(),
  canGoForward: false,
  onGoForward: vi.fn(),
  theme: 'dark' as const,
  onToggleTheme: vi.fn(),
  canShowDuplicates: false,
};

describe('Toolbar', () => {
  it('should render theme toggle button', () => {
    render(<Toolbar {...defaultProps} />);
    expect(screen.getByTestId('theme-toggle')).toBeDefined();
  });

  it('should show Light label when in dark mode', () => {
    render(<Toolbar {...defaultProps} theme="dark" />);
    expect(screen.getByTestId('theme-toggle')).toHaveTextContent('Light');
  });

  it('should show Dark label when in light mode', () => {
    render(<Toolbar {...defaultProps} theme="light" />);
    expect(screen.getByTestId('theme-toggle')).toHaveTextContent('Dark');
  });

  it('should call onToggleTheme when theme button is clicked', () => {
    const onToggleTheme = vi.fn();
    render(<Toolbar {...defaultProps} onToggleTheme={onToggleTheme} />);
    fireEvent.click(screen.getByTestId('theme-toggle'));
    expect(onToggleTheme).toHaveBeenCalledTimes(1);
  });
});
