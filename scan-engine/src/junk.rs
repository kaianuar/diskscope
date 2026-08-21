//! Junk/cache detection adapter.
//!
//! Provides [`DEFAULT_RULES`] — the built-in set of junk directory
//! patterns — and [`find_junk`] as a convenience facade over
//! [`domain::junk::detect_junk`].

use domain::junk::{detect_junk, JunkCategory, JunkReport, JunkRule};
use domain::FileNode;

/// The default set of junk detection rules.
pub static DEFAULT_RULES: &[JunkRule] = &[
    JunkRule {
        name: "node_modules",
        dir_name: "node_modules",
        category: JunkCategory::Regenerable,
        description: "npm/pnpm/yarn dependencies (regenerable via install)",
    },
    JunkRule {
        name: "target",
        dir_name: "target",
        category: JunkCategory::BuildArtifact,
        description: "Rust build artifacts (regenerable via cargo build)",
    },
    JunkRule {
        name: "__pycache__",
        dir_name: "__pycache__",
        category: JunkCategory::Cache,
        description: "Python bytecode cache",
    },
    JunkRule {
        name: ".venv",
        dir_name: ".venv",
        category: JunkCategory::VirtualEnv,
        description: "Python virtual environment",
    },
    JunkRule {
        name: "dist",
        dir_name: "dist",
        category: JunkCategory::BuildArtifact,
        description: "Build output (regenerable)",
    },
    JunkRule {
        name: "build",
        dir_name: "build",
        category: JunkCategory::BuildArtifact,
        description: "Build output (regenerable)",
    },
    JunkRule {
        name: ".cache",
        dir_name: ".cache",
        category: JunkCategory::Cache,
        description: "General cache directory",
    },
    JunkRule {
        name: ".next",
        dir_name: ".next",
        category: JunkCategory::BuildArtifact,
        description: "Next.js build cache",
    },
    JunkRule {
        name: ".nuxt",
        dir_name: ".nuxt",
        category: JunkCategory::BuildArtifact,
        description: "Nuxt.js build cache",
    },
    JunkRule {
        name: "Pods",
        dir_name: "Pods",
        category: JunkCategory::Regenerable,
        description: "CocoaPods dependencies (regenerable via pod install)",
    },
    JunkRule {
        name: ".gradle",
        dir_name: ".gradle",
        category: JunkCategory::Cache,
        description: "Gradle cache",
    },
    JunkRule {
        name: ".m2",
        dir_name: ".m2",
        category: JunkCategory::Cache,
        description: "Maven local repository cache",
    },
    JunkRule {
        name: ".pytest_cache",
        dir_name: ".pytest_cache",
        category: JunkCategory::Cache,
        description: "Pytest cache",
    },
    JunkRule {
        name: ".mypy_cache",
        dir_name: ".mypy_cache",
        category: JunkCategory::Cache,
        description: "Mypy type-checking cache",
    },
];

/// Find all junk in the given file tree using the default rules.
pub fn find_junk(root: &FileNode) -> JunkReport {
    detect_junk(root, DEFAULT_RULES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_include_node_modules_and_target_in_default_rules() {
        assert!(DEFAULT_RULES.iter().any(|r| r.dir_name == "node_modules"));
        assert!(DEFAULT_RULES.iter().any(|r| r.dir_name == "target"));
    }

    #[test]
    fn should_include_python_and_build_rules() {
        assert!(DEFAULT_RULES.iter().any(|r| r.dir_name == "__pycache__"));
        assert!(DEFAULT_RULES.iter().any(|r| r.dir_name == ".venv"));
        assert!(DEFAULT_RULES.iter().any(|r| r.dir_name == "dist"));
        assert!(DEFAULT_RULES.iter().any(|r| r.dir_name == "build"));
    }
}
