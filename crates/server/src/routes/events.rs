//! Server-Sent Events endpoint.
//!
//! `GET /api/v1/events` upgrades to a `text/event-stream` that emits
//! one line per `DomainEvent` as JSON. Each iOS / LAN client opens this
//! endpoint once and refetches affected resources when it sees an event.

use axum::Router;
use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use blowup_core::infra::events::DomainEvent;
use futures_util::stream::{self, Stream};
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/events", get(events))
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|result| result.ok())
        .map(|event: DomainEvent| {
            let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
            Ok::<_, Infallible>(Event::default().event(event.as_str()).data(data))
        });
    // Start with a hello event so the client knows the stream is live.
    let hello = stream::once(async {
        Ok::<_, Infallible>(Event::default().event("hello").data("blowup-server"))
    });
    let merged = hello.chain(stream);
    Sse::new(merged).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}
