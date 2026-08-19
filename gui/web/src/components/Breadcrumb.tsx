// Breadcrumb: clickable path segments for directory navigation.

export interface BreadcrumbProps {
  path: string | null;
  rootPath: string;
  onNavigate: (path: string) => void;
}

export function Breadcrumb({ path, rootPath, onNavigate }: BreadcrumbProps) {
  const isWindows = rootPath.includes('\\') || /^[A-Z]:\\/i.test(rootPath);
  const sep = isWindows ? '\\' : '/';

  // Normalize: strip trailing separator (except for root-only paths like "/" or "C:\").
  const displayPath = path ?? rootPath;
  const isRoot = !displayPath || displayPath === rootPath;

  const rootLabel = rootPath === '/' ? '/' : rootPath;

  if (isRoot) {
    return (
      <div className="breadcrumb" data-testid="breadcrumb">
        <span className="breadcrumb-seg breadcrumb-root breadcrumb-current">
          {rootLabel}
        </span>
      </div>
    );
  }

  // Split into segments relative to rootPath.
  const relative = displayPath.startsWith(rootPath)
    ? displayPath.slice(rootPath.length)
    : displayPath;
  const segments = relative.split(sep).filter(Boolean);

  const parts: { label: string; fullPath: string }[] = [];
  let cumulative = rootPath;
  // Avoid double separator when rootPath already ends with separator.
  for (const seg of segments) {
    if (cumulative.endsWith(sep)) {
      cumulative += seg;
    } else {
      cumulative += sep + seg;
    }
    parts.push({ label: seg, fullPath: cumulative });
  }

  return (
    <div className="breadcrumb" data-testid="breadcrumb">
      <button
        type="button"
        className="breadcrumb-seg"
        onClick={() => onNavigate(rootPath)}
      >
        {rootLabel}
      </button>
      {parts.map((p, i) => (
        <span key={p.fullPath}>
          <span className="breadcrumb-sep">{sep}</span>
          <button
            type="button"
            className={`breadcrumb-seg${i === parts.length - 1 ? ' breadcrumb-current' : ''}`}
            onClick={() => onNavigate(p.fullPath)}
          >
            {p.label}
          </button>
        </span>
      ))}
    </div>
  );
}
