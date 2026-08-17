import type { ScanFilter } from "../domain";

export interface FilterBarProps {
  filter: ScanFilter;
  onChange: (filter: ScanFilter) => void;
}

const FILE_TYPES = [
  "Image",
  "Video",
  "Audio",
  "Document",
  "Code",
  "Archive",
  "Other",
] as const;

/**
 * FilterBar: size range, file type checkboxes, name pattern search.
 */
export function FilterBar({ filter, onChange }: FilterBarProps) {
  const toggleType = (t: string) => {
    const current = filter.file_types ?? [];
    const next = current.includes(t)
      ? current.filter((x) => x !== t)
      : [...current, t];
    onChange({ ...filter, file_types: next.length > 0 ? next : undefined });
  };

  return (
    <div className="filter-bar" data-testid="filter-bar">
      <div className="filter-group">
        <label>
          Min size
          <input
            type="text"
            placeholder="1MB"
            value={filter.min_size ?? ""}
            onChange={(e) =>
              onChange({
                ...filter,
                min_size: e.target.value ? Number(e.target.value) : undefined,
              })
            }
            className="filter-input"
          />
        </label>
        <label>
          Max size
          <input
            type="text"
            placeholder="1GB"
            value={filter.max_size ?? ""}
            onChange={(e) =>
              onChange({
                ...filter,
                max_size: e.target.value ? Number(e.target.value) : undefined,
              })
            }
            className="filter-input"
          />
        </label>
      </div>

      <div className="filter-group">
        <label>
          Name
          <input
            type="text"
            placeholder="*.log"
            value={filter.name_pattern ?? ""}
            onChange={(e) =>
              onChange({
                ...filter,
                name_pattern: e.target.value || undefined,
              })
            }
            className="filter-input"
          />
        </label>
      </div>

      <div className="filter-group filter-types">
        {FILE_TYPES.map((t) => (
          <label key={t} className="filter-type-label">
            <input
              type="checkbox"
              checked={(filter.file_types ?? []).includes(t)}
              onChange={() => toggleType(t)}
            />
            {t}
          </label>
        ))}
      </div>
    </div>
  );
}
