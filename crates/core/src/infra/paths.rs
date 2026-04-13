//! Path validation helpers shared by adapters.
//!
//! The library index, S3 exports, and subtitle tools all end up
//! appending user-owned strings (from `.index.json`, request bodies,
//! etc.) to a trusted root directory. This module centralises the
//! check that the tail is actually a plain relative path, so callers
//! don't end up escaping the root via `..` or an absolute prefix.

use std::path::{Component, Path};

/// Returns true iff `rel` is a relative path composed only of normal
/// components (no `..`, no absolute prefix, no drive letter / Windows
/// verbatim prefix). Empty paths and `.`-only paths are rejected.
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
            _ => return false,
        }
    }
    saw_any
}

/// Returns true iff `candidate` — after canonicalization — is `root`
/// itself or lives strictly beneath it. Used to guard destructive
/// filesystem operations against paths that symlink or walk outside
/// the expected scope.
///
/// If either path fails to canonicalize (e.g. doesn't exist yet) the
/// function returns `false`, so callers can fail safe.
pub fn is_within_root(candidate: &Path, root: &Path) -> bool {
    let Ok(candidate_canon) = candidate.canonicalize() else {
        return false;
    };
    let Ok(root_canon) = root.canonicalize() else {
        return false;
    };
    candidate_canon.starts_with(&root_canon)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn curdir_with_normal_ok() {
        assert!(is_safe_relative_path("./Director/123"));
    }

    #[test]
    fn within_root_trivially_true() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        assert!(is_within_root(&sub, tmp.path()));
    }

    #[test]
    fn within_root_false_for_sibling() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        assert!(!is_within_root(&b, &a));
    }

    #[test]
    fn within_root_rejects_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing");
        assert!(!is_within_root(&missing, tmp.path()));
    }
}
