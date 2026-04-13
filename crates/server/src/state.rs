//! HTTP-adapter-flavored alias for [`blowup_core::AppContext`].
//!
//! The axum router used to define its own `AppState` struct with a
//! duplicate set of fields. Every time a shared resource was added
//! it had to be added in two places — core's wiring and server's
//! wiring — which was bug-prone.
//!
//! Now both sides share the same struct from core. This file exists
//! only so server handlers can keep the short `crate::state::AppState`
//! import path instead of having to reach into `blowup_core`.

pub use blowup_core::context::AppContext as AppState;
