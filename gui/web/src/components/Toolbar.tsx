// Toolbar: scan status, progress, cancel + rescan.

export interface ToolbarProps {
  scanning: boolean;
  progress: number | null;
  onCancel: () => void;
  onRescan: () => void;
}

export function Toolbar({ scanning, progress, onCancel, onRescan }: ToolbarProps) {
  return (
    <div className="toolbar" data-testid="toolbar">
      {scanning ? (
        <>
          <span className="toolbar-status">Scanning… {progress ?? 0}%</span>
          <button data-testid="cancel-scan" onClick={onCancel}>
            Cancel
          </button>
        </>
      ) : (
        <button data-testid="rescan" onClick={onRescan}>
          Rescan
        </button>
      )}
    </div>
  );
}
