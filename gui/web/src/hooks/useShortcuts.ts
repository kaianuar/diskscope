// Keyboard shortcuts: arrows/enter/backspace navigate, Delete moves to
// trash, Cmd/Ctrl+Z undoes. Pure hook over a document listener so tests
// can drive it with synthetic KeyboardEvents.

import { useEffect } from 'react';

export interface ShortcutHandlers {
  /** ArrowUp / ArrowDown navigation. */
  onMove: (delta: number) => void;
  /** Enter / ArrowRight: descend into a focused directory. */
  onEnter: () => void;
  /** Backspace / ArrowLeft: ascend to the parent. */
  onBack: () => void;
  /** Delete: move the focused entry to trash. */
  onDelete: () => void;
  /** Cmd/Ctrl+Z: undo the last trash move. */
  onUndo: () => void;
}

export function useShortcuts(handlers: ShortcutHandlers): void {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent): void => {
      const target = e.target as HTMLElement | null;
      // Never hijack typing in inputs/textareas.
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) return;

      const mod = e.metaKey || e.ctrlKey;

      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          handlers.onMove(1);
          break;
        case 'ArrowUp':
          e.preventDefault();
          handlers.onMove(-1);
          break;
        case 'Enter':
        case 'ArrowRight':
          e.preventDefault();
          handlers.onEnter();
          break;
        case 'Backspace':
        case 'ArrowLeft':
          e.preventDefault();
          handlers.onBack();
          break;
        case 'Delete':
          e.preventDefault();
          handlers.onDelete();
          break;
        case 'z':
        case 'Z':
          if (mod) {
            e.preventDefault();
            handlers.onUndo();
          }
          break;
        default:
          break;
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [handlers]);
}
