// Sidebar: pick a directory to scan + quick-scan shortcuts.

import { useRef } from 'react';

export interface QuickPath {
  label: string;
  path: string;
}

export interface SidebarProps {
  scanning: boolean;
  onScan: (path: string) => void;
  quickPaths?: QuickPath[];
}

export function Sidebar({ scanning, onScan, quickPaths }: SidebarProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);

  const handleScan = (): void => {
    const value = inputRef.current?.value.trim();
    if (!value) return;
    onScan(value);
  };

  return (
    <div className="sidebar" data-testid="sidebar">
      <h2>DiskScope</h2>
      <input
        ref={inputRef}
        data-testid="scan-path-input"
        type="text"
        placeholder="/path/to/scan"
        disabled={scanning}
      />
      <button data-testid="start-scan" onClick={handleScan} disabled={scanning}>
        Scan
      </button>
      {quickPaths && quickPaths.length > 0 && (
        <div className="quick-paths" data-testid="quick-paths">
          <span className="quick-paths-label">Quick scan</span>
          {quickPaths.map((qp) => (
            <button
              key={qp.path}
              type="button"
              className="quick-path-btn"
              data-testid={`quick-${qp.label.toLowerCase()}`}
              disabled={scanning}
              onClick={() => onScan(qp.path)}
            >
              {qp.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
