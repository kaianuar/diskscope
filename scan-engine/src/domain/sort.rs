/// Key and direction for sorting file tree children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    /// Sort by size, largest first.
    SizeDesc,
    /// Sort by size, smallest first.
    SizeAsc,
    /// Sort by name, A→Z.
    NameAsc,
    /// Sort by name, Z→A.
    NameDesc,
}
