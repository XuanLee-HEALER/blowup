//! Skill bridge: a Unix-domain-socket axum listener that shares the
//! same router as the in-process blowup-server, but is gated by a
//! session-only Settings switch instead of a bearer token.
//!
//! See `docs/superpowers/specs/2026-04-14-skill-bridge-design.md`
//! for the design rationale.

pub mod state;
