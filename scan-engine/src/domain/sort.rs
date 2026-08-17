/// Key and direction for sorting file tree children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    SizeDesc,
    SizeAsc,
    NameAsc,
    NameDesc,
}
