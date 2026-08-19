// Filter panel: size range, file type, age, name pattern.
// Debounces changes so rapid typing doesn't spam rescans.

import { useEffect, useRef, useState } from 'react';
import type { FileTypeName, Filter } from '../ipc';

export interface FilterPanelProps {
  value: Filter | undefined;
  onChange: (filter: Filter | undefined) => void;
}

// Matches useScan's debounce contract: rapid input updates coalesce.
export const FILTER_DEBOUNCE_MS = 300;

const TYPE_OPTIONS: FileTypeName[] = [
  'audio',
  'video',
  'image',
  'document',
  'code',
  'archive',
];

export function FilterPanel({ value, onChange }: FilterPanelProps) {
  const [draft, setDraft] = useState<Filter | undefined>(value);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  const commit = (next: Filter): void => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      const hasAny =
        next.minSize !== undefined ||
        next.maxSize !== undefined ||
        (next.fileTypes && next.fileTypes.length > 0) ||
        next.namePattern !== undefined ||
        next.maxAge !== undefined;
      onChange(hasAny ? next : undefined);
    }, FILTER_DEBOUNCE_MS);
  };

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  const toggleType = (t: FileTypeName): void => {
    const current = draft?.fileTypes ?? [];
    const next = current.includes(t) ? current.filter((x) => x !== t) : [...current, t];
    commit({ ...draft, fileTypes: next });
    setDraft({ ...draft, fileTypes: next });
  };

  return (
    <div className="filter-panel" data-testid="filter-panel">
      <input
        data-testid="filter-name"
        placeholder="Name pattern…"
        value={draft?.namePattern ?? ''}
        onChange={(e) => {
          const v = e.target.value;
          const next = { ...draft, namePattern: v === '' ? undefined : v };
          setDraft(next);
          commit(next);
        }}
      />
      <input
        data-testid="filter-min-size"
        type="number"
        placeholder="Min size (B)"
        value={draft?.minSize ?? ''}
        onChange={(e) => {
          const v = e.target.value;
          const next = { ...draft, minSize: v === '' ? undefined : Number(v) };
          setDraft(next);
          commit(next);
        }}
      />
      <input
        data-testid="filter-max-size"
        type="number"
        placeholder="Max size (B)"
        value={draft?.maxSize ?? ''}
        onChange={(e) => {
          const v = e.target.value;
          const next = { ...draft, maxSize: v === '' ? undefined : Number(v) };
          setDraft(next);
          commit(next);
        }}
      />
      {TYPE_OPTIONS.map((t) => (
        <label key={t} className="filter-type">
          <input
            type="checkbox"
            checked={(draft?.fileTypes ?? []).includes(t)}
            onChange={() => toggleType(t)}
          />
          {t}
        </label>
      ))}
    </div>
  );
}
