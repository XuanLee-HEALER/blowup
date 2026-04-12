//! Cross-adapter event bus.
//!
//! A thin wrapper around `tokio::sync::broadcast` plus a typed
//! `DomainEvent` enum. Services and adapters publish events via
//! `EventBus::publish`; subscribers (Tauri's re-emitter, the server's
//! SSE endpoint) consume them via `EventBus::subscribe`.
//!
//! The channel is fire-and-forget — send errors are swallowed so a
//! service call never fails just because no one is listening.

use serde::Serialize;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 128;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind")]
pub enum DomainEvent {
    #[serde(rename = "downloads:changed")]
    DownloadsChanged,
    #[serde(rename = "library:changed")]
    LibraryChanged,
    #[serde(rename = "entries:changed")]
    EntriesChanged,
    #[serde(rename = "config:changed")]
    ConfigChanged,
}

impl DomainEvent {
    /// The frontend string identifier (matches the names Tauri wrappers
    /// have been emitting via `app.emit("...", ())`).
    pub fn as_str(&self) -> &'static str {
        match self {
            DomainEvent::DownloadsChanged => "downloads:changed",
            DomainEvent::LibraryChanged => "library:changed",
            DomainEvent::EntriesChanged => "entries:changed",
            DomainEvent::ConfigChanged => "config:changed",
        }
    }
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<DomainEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        Self { tx }
    }

    /// Publish an event. Errors are ignored — a missing subscriber
    /// should not break a service call.
    pub fn publish(&self, event: DomainEvent) {
        let _ = self.tx.send(event);
    }

    /// Get a fresh receiver. Each subscriber starts from the next
    /// published event.
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_subscribe_roundtrip() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.publish(DomainEvent::EntriesChanged);
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, DomainEvent::EntriesChanged));
    }

    #[tokio::test]
    async fn publish_without_subscriber_is_fine() {
        let bus = EventBus::new();
        bus.publish(DomainEvent::DownloadsChanged);
    }

    #[test]
    fn event_name_matches_frontend() {
        assert_eq!(DomainEvent::DownloadsChanged.as_str(), "downloads:changed");
        assert_eq!(DomainEvent::LibraryChanged.as_str(), "library:changed");
        assert_eq!(DomainEvent::EntriesChanged.as_str(), "entries:changed");
        assert_eq!(DomainEvent::ConfigChanged.as_str(), "config:changed");
    }
}
