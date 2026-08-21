// Duplicates view: header, summary, list of group cards.

import type { DuplicateReport } from '../ipc';
import { formatSize } from '../lib/formatSize';
import { DuplicateGroupCard } from './DuplicateGroupCard';

export interface DuplicatesViewProps {
  report: DuplicateReport | null;
  loading: boolean;
  error: string | null;
  onBack: () => void;
  onDelete: (paths: string[]) => Promise<void>;
  onReveal: (path: string) => void;
  onOpen: (path: string) => void;
}

export function DuplicatesView({
  report,
  loading,
  error,
  onBack,
  onDelete,
  onReveal,
  onOpen,
}: DuplicatesViewProps) {
  return (
    <div className="dupes-view" data-testid="dupes-view">
      <div className="dupes-header">
        <button
          type="button"
          className="dupes-back-btn"
          data-testid="dupes-back"
          onClick={onBack}
        >
          ← Back to files
        </button>
        <h2 className="dupes-title">Duplicates</h2>
      </div>

      {loading && (
        <div className="dupes-loading" data-testid="dupes-loading">
          Scanning for duplicates…
        </div>
      )}

      {error && (
        <div className="dupes-error" data-testid="dupes-error">
          {error}
        </div>
      )}

      {!loading && !error && report && report.groups.length === 0 && (
        <div className="dupes-empty" data-testid="dupes-empty">
          No duplicate files found
        </div>
      )}

      {!loading && !error && report && report.groups.length > 0 && (
        <>
          <div className="dupes-summary" data-testid="dupes-summary">
            {report.groups.length} duplicate groups · {formatSize(report.totalRecoverable)} recoverable
          </div>
          <div className="dupes-list">
            {report.groups.map((group) => (
              <DuplicateGroupCard
                key={group.hash}
                group={group}
                onDelete={onDelete}
                onReveal={onReveal}
                onOpen={onOpen}
              />
            ))}
          </div>
        </>
      )}
    </div>
  );
}
