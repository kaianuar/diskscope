// Tests for the keyboard shortcut wiring.

import { describe, expect, it, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useShortcuts } from '../useShortcuts';

function key(key: string, opts: { metaKey?: boolean; ctrlKey?: boolean } = {}): KeyboardEvent {
  return new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...opts });
}

describe('useShortcuts', () => {
  it('should trigger navigation, delete and undo handlers on keys', () => {
    const handlers = {
      onMove: vi.fn(),
      onEnter: vi.fn(),
      onBack: vi.fn(),
      onDelete: vi.fn(),
      onUndo: vi.fn(),
    };
    renderHook(() => useShortcuts(handlers));

    act(() => {
      document.dispatchEvent(key('ArrowDown'));
      document.dispatchEvent(key('ArrowUp'));
      document.dispatchEvent(key('Enter'));
      document.dispatchEvent(key('Backspace'));
      document.dispatchEvent(key('Delete'));
      document.dispatchEvent(key('z', { ctrlKey: true }));
    });

    expect(handlers.onMove).toHaveBeenCalledTimes(2);
    expect(handlers.onMove).toHaveBeenCalledWith(1);
    expect(handlers.onMove).toHaveBeenCalledWith(-1);
    expect(handlers.onEnter).toHaveBeenCalledTimes(1);
    expect(handlers.onBack).toHaveBeenCalledTimes(1);
    expect(handlers.onDelete).toHaveBeenCalledTimes(1);
    expect(handlers.onUndo).toHaveBeenCalledTimes(1);
  });

  it('should ignore keys typed into inputs', () => {
    const handlers = {
      onMove: vi.fn(),
      onEnter: vi.fn(),
      onBack: vi.fn(),
      onDelete: vi.fn(),
      onUndo: vi.fn(),
    };
    renderHook(() => useShortcuts(handlers));

    const input = document.createElement('input');
    document.body.appendChild(input);
    try {
      input.dispatchEvent(key('Delete'));
      input.dispatchEvent(key('ArrowDown'));
      expect(handlers.onDelete).not.toHaveBeenCalled();
      expect(handlers.onMove).not.toHaveBeenCalled();
    } finally {
      document.body.removeChild(input);
    }
  });
});
