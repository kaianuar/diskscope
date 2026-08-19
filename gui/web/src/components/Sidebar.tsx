// Sidebar: pick a directory to scan.

import { useRef } from 'react';

export interface SidebarProps {
  scanning: boolean;
  onScan: (path: string) => void;
}

export function Sidebar({ scanning, onScan }: SidebarProps) {
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
    </div>
  );
}
