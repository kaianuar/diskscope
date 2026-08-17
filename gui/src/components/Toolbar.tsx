export interface ToolbarProps {
  /** Whether we can go up (not at root). */
  canGoUp: boolean;
  /** Whether a scan is currently loading. */
  loading: boolean;
  onGoUp: () => void;
  onRefresh: () => void;
  /** Breadcrumb labels for display. */
  breadcrumbs: string[];
  onBreadcrumbClick: (index: number) => void;
}

/**
 * Toolbar: back/up navigation, breadcrumbs, refresh button.
 */
export function Toolbar({
  canGoUp,
  loading,
  onGoUp,
  onRefresh,
  breadcrumbs,
  onBreadcrumbClick,
}: ToolbarProps) {
  return (
    <div className="toolbar" data-testid="toolbar">
      <button
        onClick={onGoUp}
        disabled={!canGoUp}
        title="Go up (Backspace)"
        className="toolbar-btn"
      >
        ← Up
      </button>
      <button
        onClick={onRefresh}
        disabled={loading}
        title="Refresh scan"
        className="toolbar-btn"
      >
        ↻
      </button>
      <div className="toolbar-breadcrumbs" data-testid="breadcrumbs">
        {breadcrumbs.map((crumb, i) => (
          <span key={i}>
            {i > 0 && <span className="breadcrumb-sep">/</span>}
            <button
              className="breadcrumb-link"
              onClick={() => onBreadcrumbClick(i)}
            >
              {crumb}
            </button>
          </span>
        ))}
      </div>
    </div>
  );
}
