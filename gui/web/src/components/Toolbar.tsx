// Toolbar: scan status, progress, cancel + rescan, navigation buttons.

export interface ToolbarProps {
  scanning: boolean;
  progress: number | null;
  onCancel: () => void;
  onRescan: () => void;
  canGoUp: boolean;
  onGoUp: () => void;
  canGoBack: boolean;
  onGoBack: () => void;
  canGoForward: boolean;
  onGoForward: () => void;
}

export function Toolbar({
  scanning,
  progress,
  onCancel,
  onRescan,
  canGoUp,
  onGoUp,
  canGoBack,
  onGoBack,
  canGoForward,
  onGoForward,
}: ToolbarProps) {
  return (
    <div className="toolbar" data-testid="toolbar">
      <button
        type="button"
        className="toolbar-nav-btn"
        data-testid="back-btn"
        disabled={!canGoBack}
        onClick={onGoBack}
        title="Back"
      >
        ← Back
      </button>
      <button
        type="button"
        className="toolbar-nav-btn"
        data-testid="forward-btn"
        disabled={!canGoForward}
        onClick={onGoForward}
        title="Forward"
      >
        → Forward
      </button>
      <button
        type="button"
        className="toolbar-up-btn"
        data-testid="up-btn"
        disabled={!canGoUp}
        onClick={onGoUp}
        title="Up"
      >
        ↑ Up
      </button>
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
