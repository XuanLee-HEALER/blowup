//! Long-running background task tracking.
//!
//! Some operations (subtitle alignment, future audio extract, HLS
//! transmux, …) take seconds to minutes to complete. The Tauri IPC
//! command pattern of "await the result on the calling promise" is
//! brittle: if the frontend unmounts mid-flight (page navigation),
//! the promise is orphaned and the result is lost, while the backend
//! tokio task keeps running to completion. The user sees a stuck UI.
//!
//! This module is the authoritative store of in-flight / recently
//! completed tasks. Both Tauri commands and server routes:
//!
//!   1. Call `service::run_*` helpers which immediately spawn the
//!      real work in a background tokio task and insert a Running
//!      record into `TaskRegistry`.
//!   2. Return the task id to the caller without waiting.
//!   3. When the spawned task finishes, the registry is updated to
//!      Completed / Failed and `DomainEvent::TasksChanged` is
//!      published on the shared `EventBus`.
//!   4. Frontends (Tauri WebView / iOS) re-query `list` on
//!      `tasks:changed` to refresh their UI. Late subscribers
//!      (components that just mounted) get the current state by
//!      querying once on mount.

pub mod model;
pub mod registry;

pub use model::{TaskKind, TaskRecord, TaskStatus};
pub use registry::TaskRegistry;
