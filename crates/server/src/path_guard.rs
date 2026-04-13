//! Path validation helpers for routes that take user-supplied
//! relative paths from `.index.json` or request bodies.
//!
//! The goal is to reject anything that could escape the library root
//! via `..`, an absolute path, or a Windows-style drive prefix.

use std::path::{Component, Path};

/// Returns true iff `rel` is a relative path composed only of normal
/// components (no `..`, no absolute prefix, no drive letter). Empty
/// paths are rejected.
pub fn is_safe_relative_path(rel: &str) -> bool {
    if rel.is_empty() {
        return false;
    }
    let p = Path::new(rel);
    let mut saw_any = false;
    for component in p.components() {
        match component {
            Component::Normal(_) => saw_any = true,
            Component::CurDir => continue,
            // Reject ParentDir, RootDir, Prefix.
            _ => return false,
        }
    }
    saw_any
}

#[cfg(test)]
mod tests {
    use super::is_safe_relative_path;

    #[test]
    fn plain_relative_path_ok() {
        assert!(is_safe_relative_path("Director/123"));
        assert!(is_safe_relative_path("some/nested/dir"));
    }

    #[test]
    fn absolute_path_rejected() {
        assert!(!is_safe_relative_path("/etc/passwd"));
    }

    #[test]
    fn parent_dir_rejected() {
        assert!(!is_safe_relative_path("../etc"));
        assert!(!is_safe_relative_path("a/../b"));
        assert!(!is_safe_relative_path("foo/.."));
    }

    #[test]
    fn empty_rejected() {
        assert!(!is_safe_relative_path(""));
    }

    #[test]
    fn curdir_only_rejected() {
        assert!(!is_safe_relative_path("."));
        assert!(!is_safe_relative_path("./"));
    }

    #[test]
    fn curdir_with_normal_ok() {
        assert!(is_safe_relative_path("./Director/123"));
    }
}
