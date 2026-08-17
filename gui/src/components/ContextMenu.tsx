export interface ContextMenuProps {
  x: number;
  y: number;
  visible: boolean;
  onOpenExplorer: () => void;
  onCopyPath: () => void;
  onCopySize: () => void;
  onDelete: () => void;
  onClose: () => void;
}

/**
 * ContextMenu: right-click actions for file entries.
 */
export function ContextMenu({
  x,
  y,
  visible,
  onOpenExplorer,
  onCopyPath,
  onCopySize,
  onDelete,
  onClose,
}: ContextMenuProps) {
  if (!visible) return null;

  return (
    <>
      <div className="context-menu-overlay" onClick={onClose} />
      <div
        className="context-menu"
        data-testid="context-menu"
        style={{ left: x, top: y }}
        role="menu"
      >
        <button role="menuitem" onClick={onOpenExplorer}>
          Open in file explorer
        </button>
        <button role="menuitem" onClick={onCopyPath}>
          Copy path
        </button>
        <button role="menuitem" onClick={onCopySize}>
          Copy size
        </button>
        <hr />
        <button role="menuitem" onClick={onDelete} className="danger">
          Move to trash
        </button>
      </div>
    </>
  );
}
