// Path utilities: OS-aware parent extraction.
//
// Both Windows (`\`) and POSIX (`/`) paths are supported. The separator
// is inferred from the path content, not the running OS, so paths
// received from the Rust backend work correctly in the browser.

/**
 * Return the parent directory of `p`.
 *
 * - `parentOf('/a/b')` → `'/a'`
 * - `parentOf('C:\\a\\b')` → `'C:\\a'`
 * - `parentOf('/a')` → `'/'`  (root)
 * - Trailing separators are stripped before computing the parent.
 */
export function parentOf(p: string): string {
  const trimmed = p.replace(/[/\\]+$/, '');
  const sep: '\\' | '/' = trimmed.includes('\\') ? '\\' : '/';
  const idx = trimmed.lastIndexOf(sep);
  if (idx <= 0) {
    // POSIX root, or Windows drive root ('C:\a' → 'C:\').
    if (sep === '\\') {
      const root = trimmed.slice(0, 2);
      return root.length === 2 && /^[A-Za-z]:$/.test(root) ? `${root}\\` : root || '\\';
    }
    return '/';
  }
  const parent = trimmed.slice(0, idx);
  // 'C:\a' → parent 'C:' → normalize to drive root 'C:\'.
  if (/^[A-Za-z]:$/.test(parent)) return `${parent}\\`;
  return parent;
}
