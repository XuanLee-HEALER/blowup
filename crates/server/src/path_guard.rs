//! Thin re-export of [`blowup_core::infra::paths`] so server route
//! handlers can keep using the short `crate::path_guard::` prefix.

pub use blowup_core::infra::paths::{is_safe_relative_path, is_within_root};
