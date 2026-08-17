import { useEffect, useCallback } from "react";

export interface KeyboardHandlers {
  onArrowUp?: () => void;
  onArrowDown?: () => void;
  onArrowLeft?: () => void;
  onArrowRight?: () => void;
  onEnter?: () => void;
  onBackspace?: () => void;
  onDelete?: () => void;
  onUndo?: () => void;
  onSearch?: () => void;
}

/**
 * Registers global keyboard shortcuts for file navigation.
 */
export function useKeyboard(handlers: KeyboardHandlers) {
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // Don't capture when user is typing in an input
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;

      const ctrl = e.metaKey || e.ctrlKey;

      switch (e.key) {
        case "ArrowUp":
          e.preventDefault();
          handlers.onArrowUp?.();
          break;
        case "ArrowDown":
          e.preventDefault();
          handlers.onArrowDown?.();
          break;
        case "ArrowLeft":
          e.preventDefault();
          handlers.onArrowLeft?.();
          break;
        case "ArrowRight":
          e.preventDefault();
          handlers.onArrowRight?.();
          break;
        case "Enter":
          e.preventDefault();
          handlers.onEnter?.();
          break;
        case "Backspace":
          e.preventDefault();
          handlers.onBackspace?.();
          break;
        case "Delete":
          e.preventDefault();
          handlers.onDelete?.();
          break;
        case "z":
          if (ctrl) {
            e.preventDefault();
            handlers.onUndo?.();
          }
          break;
        case "f":
          if (ctrl) {
            e.preventDefault();
            handlers.onSearch?.();
          }
          break;
      }
    },
    [handlers]
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);
}
