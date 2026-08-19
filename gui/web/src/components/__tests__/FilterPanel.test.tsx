// Tests for FilterPanel debounce behaviour.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { FilterPanel, FILTER_DEBOUNCE_MS } from '../FilterPanel';

describe('FilterPanel', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('should debounce filter changes when filter input updates rapidly', () => {
    const onChange = vi.fn();
    render(<FilterPanel value={undefined} onChange={onChange} />);

    const input = screen.getByTestId('filter-name') as HTMLInputElement;

    // Simulate rapid typing: three successive value changes.
    fireEvent.change(input, { target: { value: 'a' } });
    fireEvent.change(input, { target: { value: 'ab' } });
    fireEvent.change(input, { target: { value: 'abc' } });

    // No onChange yet: debounce has not fired.
    expect(onChange).not.toHaveBeenCalled();

    // Advance past the debounce window.
    act(() => {
      vi.advanceTimersByTime(FILTER_DEBOUNCE_MS + 50);
    });

    // onChange fires exactly once with the final value.
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ namePattern: 'abc' }),
    );
  });
});
