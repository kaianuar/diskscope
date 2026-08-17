use std::fmt;

/// A size in bytes with human-readable display formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Size(pub u64);

impl Size {
    /// Create a new Size from bytes.
    pub const fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    /// Return the value in bytes.
    pub const fn bytes(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.0 as f64;
        for unit in UNITS {
            if size < 1024.0 {
                return write!(f, "{:.1} {}", size, unit);
            }
            size /= 1024.0;
        }
        write!(f, "{:.1} PB", size)
    }
}

impl From<u64> for Size {
    fn from(bytes: u64) -> Self {
        Self(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_format_bytes_when_under_1024() {
        assert_eq!(Size::new(512).to_string(), "512.0 B");
    }

    #[test]
    fn should_format_kilobytes_when_over_1024() {
        assert_eq!(Size::new(2048).to_string(), "2.0 KB");
    }

    #[test]
    fn should_format_megabytes_when_large() {
        assert_eq!(Size::new(5 * 1024 * 1024).to_string(), "5.0 MB");
    }

    #[test]
    fn should_format_gigabytes_when_very_large() {
        assert_eq!(Size::new(3 * 1024 * 1024 * 1024).to_string(), "3.0 GB");
    }

    #[test]
    fn should_format_zero_bytes() {
        assert_eq!(Size::new(0).to_string(), "0.0 B");
    }

    #[test]
    fn should_compare_sizes() {
        assert!(Size::new(100) < Size::new(200));
        assert!(Size::new(200) > Size::new(100));
        assert_eq!(Size::new(100), Size::new(100));
    }
}
