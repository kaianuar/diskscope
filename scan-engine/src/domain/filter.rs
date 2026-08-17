/// Criteria for filtering files/directories in the scan tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    /// Include only files with size >= this many bytes.
    MinSize(u64),
    /// Include only files with size <= this many bytes.
    MaxSize(u64),
    /// Include only files whose extension (case-insensitive) matches.
    Extension(String),
    /// Include only files modified within this duration from now.
    MaxAge(std::time::Duration),
    /// Include only files whose name matches this glob (`*`/`?`) pattern.
    NamePattern(String),
}
