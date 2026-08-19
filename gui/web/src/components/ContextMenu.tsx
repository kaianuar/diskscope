// Context menu: right-click actions on file entries.
// Open in explorer, copy path, copy relative path.

import { useCallback, useEffect, useRef } from 'react';
import { revealInExplorer } from '../ipc';

export interface ContextMenuProps {
  /** Absolute path of the targeted entry. */
  path: string;
  /** Root path of the current scan (for computing relative paths). */
  rootPath: string;
  /** Pixel X of the click origin (viewport coords). */
  x: number;
  /** Pixel Y of the click origin (viewport coords). */
  y: number;
  /** Close the menu (called after an action or on outside click / Escape). */
  onClose: () => void;
  /** Optional error setter from the parent. */
  onError?: (message: string) => void;
}

export function ContextMenu({ path, rootPath, x, y, onClose, onError }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement | null>(null);

  const run = useCallback(
    async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (err) {
        onError?.(String(err));
      }
      onClose();
    },
    [onClose, onError],
  );

  const handleReveal = useCallback(
    () => run(() => revealInExplorer(path)),
    [run, path],
  );

  const handleCopyPath = useCallback(
    () => run(async () => { await navigator.clipboard.writeText(path); }),
    [run, path],
  );

  const handleCopyRelative = useCallback(
    () =>
      run(async () => {
        const rel = path.startsWith(rootPath + '/')
          ? path.slice(rootPath.length + 1)
          : path;
        await navigator.clipboard.writeText(rel);
      }),
    [run, path, rootPath],
  );

  useEffect(() => {
    const onClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('mousedown', onClickOutside);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onClickOutside);
      document.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      className="context-menu"
      data-testid="context-menu"
      style={{ left: x, top: y }}
      role="menu"
    >
      <button role="menuitem" data-testid="ctx-reveal" onClick={handleReveal}>
        Reveal in Explorer
      </button>
      <button role="menuitem" data-testid="ctx-copy-path" onClick={handleCopyPath}>
        Copy Path
      </button>
      <button role="menuitem" data-testid="ctx-copy-relative" onClick={handleCopyRelative}>
        Copy Relative Path
      </button>
    </div>
  );
}
