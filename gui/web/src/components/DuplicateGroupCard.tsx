// One duplicate group card: expand/collapse, file rows with checkboxes,
// keeper marker, delete-selected action.

import { useCallback, useEffect, useRef, useState } from 'react';
import type { DuplicateGroup } from '../ipc';
import { formatSize } from '../lib/formatSize';

export interface DuplicateGroupCardProps {
  group: DuplicateGroup;
  onDelete: (paths: string[]) => void;
  onReveal: (path: string) => void;
  onOpen: (path: string) => void;
}

export function DuplicateGroupCard({
  group,
  onDelete,
  onReveal,
  onOpen,
}: DuplicateGroupCardProps) {
  const [expanded, setExpanded] = useState(true);
  const [checked, setChecked] = useState<Set<number>>(() => {
    // Default: all non-keeper files checked.
    const initial = new Set<number>();
    for (let i = 1; i < group.files.length; i++) initial.add(i);
    return initial;
  });
  const [ctxMenu, setCtxMenu] = useState<{ path: string; x: number; y: number } | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!ctxMenu) return;
    const onClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setCtxMenu(null);
    };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setCtxMenu(null); };
    document.addEventListener('mousedown', onClickOutside);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onClickOutside);
      document.removeEventListener('keydown', onKey);
    };
  }, [ctxMenu]);

  const toggleCheck = useCallback((index: number) => {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  }, []);

  const handleDelete = useCallback(() => {
    const paths = [...checked].map((i) => group.files[i]);
    if (paths.length > 0) onDelete(paths);
  }, [checked, group.files, onDelete]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, path: string) => {
      e.preventDefault();
      setCtxMenu({ path, x: e.clientX, y: e.clientY });
    },
    [],
  );

  const recoverable = group.size * (group.files.length - 1);

  return (
    <div className="dupe-group" data-testid="dupe-group">
      <div
        className="dupe-group-header"
        data-testid="dupe-group-header"
        onClick={() => setExpanded((v) => !v)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') setExpanded((v) => !v); }}
      >
        <span className="dupe-chevron" aria-hidden="true">{expanded ? '▾' : '▸'}</span>
        <span className="dupe-recoverable">{formatSize(recoverable)} recoverable</span>
        <span className="dupe-file-count">{group.files.length} files</span>
        {checked.size > 0 && (
          <button
            type="button"
            className="dupe-delete-btn"
            data-testid="dupe-delete"
            onClick={(e) => { e.stopPropagation(); handleDelete(); }}
          >
            Delete selected ({checked.size})
          </button>
        )}
      </div>
      {expanded && (
        <div className="dupe-rows">
          {group.files.map((path, i) => {
            const isKeeper = i === 0;
            return (
              <div
                key={path}
                className={`dupe-row${isKeeper ? ' is-keeper' : ''}`}
                data-testid={`dupe-row-${i}`}
                onContextMenu={(e) => handleContextMenu(e, path)}
                onDoubleClick={() => onOpen(path)}
              >
                {isKeeper ? (
                  <span className="dupe-keeper-marker" data-testid={`dupe-keeper-${i}`}>
                    ● keep
                  </span>
                ) : (
                  <input
                    type="checkbox"
                    className="dupe-check"
                    data-testid={`dupe-check-${i}`}
                    checked={checked.has(i)}
                    onChange={() => toggleCheck(i)}
                  />
                )}
                <span className="dupe-path" title={path}>{path}</span>
                <span className="dupe-size">{formatSize(group.size)}</span>
              </div>
            );
          })}
        </div>
      )}
      {ctxMenu && (
        <div
          ref={menuRef}
          className="context-menu"
          data-testid="context-menu"
          style={{ left: ctxMenu.x, top: ctxMenu.y }}
          role="menu"
        >
          <button
            role="menuitem"
            data-testid="ctx-reveal"
            onClick={() => { onReveal(ctxMenu.path); setCtxMenu(null); }}
          >
            Reveal in Explorer
          </button>
          <button
            role="menuitem"
            data-testid="ctx-open"
            onClick={() => { onOpen(ctxMenu.path); setCtxMenu(null); }}
          >
            Open
          </button>
          <button
            role="menuitem"
            data-testid="ctx-copy-path"
            onClick={() => { void navigator.clipboard.writeText(ctxMenu.path); setCtxMenu(null); }}
          >
            Copy Path
          </button>
        </div>
      )}
    </div>
  );
}
