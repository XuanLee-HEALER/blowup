//! blowup-mcp library exports.
//!
//! The bridge binary entry is in `main.rs`; this lib target exists
//! so the desktop Tauri side can call `socket::resolve_socket_path()`
//! to agree on the socket location, and so the test harness can
//! exercise the modules in-process.

pub mod client;
pub mod error;
pub mod service;
pub mod socket;
