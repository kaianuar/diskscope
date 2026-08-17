import { useState } from "react";
import { humanSize } from "../utils";
import type { FileNode } from "../domain";

export interface SidebarProps {
  /** Callback when user picks a directory to scan. */
  onScan: (path: string) => void;
  /** Most recent scan result root node. */
  result: FileNode | null;
  /** Total bytes of the last scan. */
  totalSize: number;
}

/**
 * Sidebar: directory picker input, recent scans, and quick stats.
 */
export function Sidebar({ onScan, result, totalSize }: SidebarProps) {
  const [path, setPath] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (path.trim()) onScan(path.trim());
  };

  return (
    <aside className="sidebar" data-testid="sidebar">
      <form onSubmit={handleSubmit} className="sidebar-form">
        <label htmlFor="scan-path" className="sidebar-label">
          Directory
        </label>
        <input
          id="scan-path"
          type="text"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          placeholder="/home/user"
          className="sidebar-input"
        />
        <button type="submit" className="sidebar-button">
          Scan
        </button>
      </form>

      {result && (
        <div className="sidebar-stats" data-testid="sidebar-stats">
          <div className="stat">
            <span className="stat-label">Path</span>
            <span className="stat-value">{result.path}</span>
          </div>
          <div className="stat">
            <span className="stat-label">Total</span>
            <span className="stat-value">{humanSize(totalSize)}</span>
          </div>
        </div>
      )}
    </aside>
  );
}
