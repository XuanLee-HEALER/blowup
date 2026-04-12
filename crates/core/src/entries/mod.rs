//! Knowledge base domain. "Entries" is the unified model — films,
//! people, genres, concepts are all entries distinguished only by
//! user-applied tags (see docs/REFACTOR.md D2).
//!
//! The graph view (`graph::get_graph_data`) is a derived query over
//! entries + relations, not a separate domain.

pub mod graph;
pub mod model;
pub mod service;

pub use model::{
    EntryDetail, EntryRow, EntrySummary, GraphData, GraphLink, GraphNode, RelationEntry,
};
