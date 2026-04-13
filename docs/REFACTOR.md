# Workspace Refactor — core / server / tauri

> **Status**: steps 1–5 merged to `main` (2026-04-13). A follow-up
> pass addressed review findings — auth + path safety on the axum
> server (blocker fixes), plus the non-blocker cleanup list below.
> Steps 6 (iOS client) and 7 (static libav) remain open.
>
> **Goal**: split the single `src-tauri` crate into a `blowup-core` library plus
> two thin adapters (`blowup-server`, `blowup-tauri`) so that a future native
> iOS/iPadOS client can share the same Rust business logic via HTTP.

## Review follow-up (2026-04-13)

The refactor landed in one large branch. An independent review
flagged 2 blockers and 18 non-blocker items; both blockers + every
non-blocker were resolved in the same push to `main`.

**Blockers fixed before merge**:

1. `crates/core/migrations/001_initial.sql` header comment had been
   rewritten during the workspace split. Since sqlx computes
   migration checksums over the entire file (comments included), the
   rewrite produced `MigrateError::VersionMismatch(1)` on every
   pre-existing install. Reverted so the file is byte-identical to
   pre-refactor `src-tauri/migrations/001_initial.sql`.
2. The axum server's `CorsLayer::very_permissive()` + zero-auth
   combination meant any web page the user visited could POST to
   `localhost:17690` and (via `import/config`, `subtitle/shift`,
   `library/resources`, `library/index/{id}` delete, …) write
   arbitrary files. Replaced with: drop the CORS layer entirely
   (no `Access-Control-Allow-Origin` → browsers block preflight) +
   mandatory `Authorization: Bearer <token>` middleware on every
   route, token resolved from `$BLOWUP_SERVER_TOKEN` or generated
   randomly per-session and logged at startup. `.index.json`
   entries are also validated against `..`-traversal before being
   joined with the library root.

**Non-blockers fixed after merge** (each landed as its own `fix:` /
`refactor:` commit, tagged N1..N18):

- N1 Tauri EventBus→app.emit forwarder handles `Lagged` without
  exiting the forwarder task.
- N2 `TaskRegistry` uses a per-start `generation` counter so a
  dismissed+restarted task's old spawned future can't clobber the
  new record. `TaskKind::id()` is also namespaced per variant so
  align-to-audio and align-to-video on the same SRT don't collide.
- N3 Download progress monitor moved to
  `core::workflows::download_monitor` and used by both the Tauri
  commands and the standalone server routes, so standalone mode
  actually transitions downloads out of `downloading`.
- N4 + N16 + N17 path-safety helpers (`infra::paths::{
  is_safe_relative_path, is_within_root}`) applied at every site
  that joins a user-controlled string into the library root.
- N5 destructive fs operations log on failure instead of
  silently swallowing the error.
- N6 `AppContext` moved to core; `blowup_server::AppState` is now
  a type alias over it, and the Tauri adapter constructs the same
  struct at startup — no more duplicate wiring when a new shared
  resource is added.
- N7 new `core::workflows/` module. `subtitle_align` and
  `download_monitor` migrated there; remaining cross-domain
  imports (`tmdb → library`, `media → library`, `torrent →
  library`) are documented as "`LibraryIndex` is an infra type"
  in `core/src/library/mod.rs` rather than hiding them.
- N8 server error layer no longer string-matches Chinese substrings
  to classify 404s. Core services tag status-relevant errors via
  `core::error::status::{not_found, bad_request}` prefixes; the
  axum adapter `strip_prefix`-matches instead.
- N9 17 shadow dependencies removed from `crates/tauri/Cargo.toml`.
- N10 dead code with `panic!()` in non-test path deleted.
- N11 `credits_put` degrades to `warn + drop` when the cache
  wasn't initialised, matching `credits_get`'s lenient behavior.
- N12 audio-peaks cache is invalidated when the source mtime is
  newer than the sidecar; writes go through `rename`-atomic tmp
  files so parallel calls never truncate each other.
- N13 `crates/server/tests/smoke.rs` — 11 smoke tests covering
  auth (200/401 for every variant of bad/missing/wrong token),
  404 on unknown routes + nonexistent resource, and fresh-install
  empty-array responses from the read paths.
- N14 (folded into N17).
- N15 every `std::sync::{Mutex, RwLock}.*().unwrap()` site
  migrated to `parking_lot` locks (no poisoning → no unwrap).
- N18 — this section.

See commit range `b4f1938..HEAD` on `main` for the implementation.

---

## Why

Today blowup is a single-process Tauri app. A native iOS client cannot host
the torrent engine, ffmpeg processing, or the film file library — so iOS
must be a **thin client** talking to a backend that owns all of that. To
avoid maintaining two copies of business logic, the Rust backend is split:

- **`blowup-core`** — pure business logic (torrent, subtitle, tmdb, library,
  entries, media, audio, export, cross-domain workflows). Zero coupling to
  Tauri or HTTP.
- **`blowup-server`** — `axum` HTTP server wrapping `blowup-core`. Runs
  headless on a NAS / Mac mini, or in-process inside the Tauri app for
  LAN-side iPad access.
- **`blowup-tauri`** — the existing desktop app, reduced to a thin adapter
  over `blowup-core` plus platform-specific pieces (mpv player, native
  windows, file dialogs).

Desktop and iOS ultimately share the same HTTP API. Desktop still has a
built-in libmpv native player for latency and offline use; iOS uses
VLCKit / AVPlayer against a streamed server endpoint.

---

## Architecture decisions

### D1. Three crates under `crates/`, `blowup-` name prefix

```
blowup/
├── Cargo.toml              # workspace root
├── crates/
│   ├── core/               # blowup-core (lib)
│   ├── server/             # blowup-server (bin)
│   └── tauri/              # blowup-tauri (bin + cdylib/staticlib/rlib)
├── src/                    # React frontend (unchanged)
├── ios/                    # future SwiftUI Xcode project
└── docs/REFACTOR.md        # this file
```

- crate names use the `blowup-` prefix — avoids clashing with Rust stdlib
  `core`, and makes dependency graphs / error messages unambiguous
- directory names are bare because they already live under `crates/`

### D2. Core organized by domain, not by layer

```
crates/core/src/
├── lib.rs              # re-exports
├── error.rs            # CoreError + Result
├── context.rs          # AppContext (see D3)
├── infra/              # cross-domain infrastructure
│   ├── db.rs           # SqlitePool init + migrations
│   ├── ffmpeg.rs       # FfmpegTool wrapper (CLI now, libav later — see D11)
│   ├── http.rs         # shared reqwest::Client
│   ├── cache.rs        # LRU cache
│   ├── events.rs       # EventBus + DomainEvent
│   └── paths.rs        # path helpers (does not assume $HOME)
├── config/             # Backend config load/save (path injected by adapter)
├── library/            # Film library — the "file owner" domain
│   ├── model.rs
│   ├── index.rs        # library_index.json persistence
│   ├── scan.rs
│   └── service.rs
├── entries/            # Knowledge base CRUD + graph_view derived query
├── tmdb/               # Stateless external API + cache
├── torrent/
│   ├── search.rs       # stateless YTS
│   ├── manager.rs      # stateful librqbit
│   └── service.rs
├── subtitle/           # parser + alass + opensubtitles + ffmpeg extraction
├── media/              # ffprobe + (future) HLS transmux
├── audio/              # audio stream extraction + waveform data
├── export/             # import/export + S3
└── workflows/          # cross-domain orchestration
    └── download_complete_to_library.rs
```

**Rules**:
- Domains **do not cross-import**. Cross-domain logic lives in `workflows/`.
- Each domain exposes one `service.rs` as its public surface. Adapters call
  `blowup_core::<domain>::service::<fn>(&ctx, ...)`.
- **Graph visualization is not a separate domain.** It is a derived query
  over entries+relations, implemented as
  `entries::service::graph_view(...)`. D3 force simulation is frontend.
- **External API search modules** (tmdb, torrent::search, opensubtitles)
  are *stateless except for cache*: they do not touch the DB and do not
  emit events. This is a property, not a module.

### D3. AppContext is a single struct

```rust
pub struct AppContext {
    pub db: SqlitePool,
    pub config: Arc<RwLock<Config>>,
    pub library_index: Arc<LibraryIndex>,
    pub torrent: Arc<TorrentManager>,
    pub ffmpeg: Arc<FfmpegTool>,
    pub http: reqwest::Client,
    pub cache: Arc<TmdbCache>,
    pub events: EventBus,
    pub paths: AppPaths,
}
```

Service functions take `&AppContext`. Adapters hold `Arc<AppContext>`:
- Tauri: `app.manage(Arc::new(ctx))`, commands take `State<Arc<AppContext>>`
- axum: `Router::new().with_state(Arc::new(ctx))`

Per-domain sub-contexts were considered and rejected: ~80% of services
need `db + config + http`, so a "clean split" would add noise without
saving anything.

### D4. Errors — typed in core, stringified at the Tauri edge

```rust
// crates/core/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("database: {0}")]            Database(#[from] sqlx::Error),
    #[error("io: {0}")]                  Io(#[from] std::io::Error),
    #[error("http: {0}")]                Http(#[from] reqwest::Error),
    #[error("not found: {0}")]           NotFound(String),
    #[error("invalid input: {0}")]       BadRequest(String),
    #[error("external tool failed: {0}")] Tool(String),
    #[error("{0}")]                      Other(String),
}
pub type Result<T> = std::result::Result<T, CoreError>;
```

- **Tauri adapter**: `.map_err(|e| e.to_string())?` — frontend still
  receives strings, no frontend changes.
- **Server adapter**: implement `IntoResponse for CoreError`, map variants
  to HTTP status (`NotFound` → 404, `BadRequest` → 400, else 500), body is
  `{"error": "..."}` JSON.

### D5. Event bus — `tokio::sync::broadcast` in core, fan-out per adapter

```rust
#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "kind")]
pub enum DomainEvent {
    DownloadsChanged,
    LibraryChanged,
    EntriesChanged,
    ConfigChanged,
}

#[derive(Clone)]
pub struct EventBus { tx: broadcast::Sender<DomainEvent> }
```

- Tauri adapter spawns a task that subscribes and re-emits as
  `app.emit("downloads:changed", ())` — existing frontend event name
  constants keep working.
- Server adapter exposes `GET /api/v1/events` as Server-Sent Events.

### D6. Naming conventions

| Object          | Convention                         | Example                                              |
| --------------- | ---------------------------------- | ---------------------------------------------------- |
| Service fn      | verb-only, no domain prefix        | `tmdb::service::search()`                            |
| Tauri command   | unchanged from today               | `tmdb_search`, `get_config`, `save_config_cmd`       |
| HTTP route      | REST, plural noun + HTTP method    | `GET /api/v1/library/items`, `DELETE /api/v1/entries/:id` |
| HTTP API prefix | `/api/v1/...`                      | leaves room for breaking changes                     |
| Domain event    | Rust enum ↔ frontend string        | `DomainEvent::DownloadsChanged` ↔ `"downloads:changed"` |

Module / directory **singular vs plural**: plural when the word names a
collection (`entries`, `downloads`, `workflows`); singular otherwise
(`subtitle`, `tmdb`, `config`, `library`, `media`, `audio`).

### D7. Service functions, not service structs

```rust
pub async fn search(ctx: &AppContext, query: &str) -> Result<Vec<Movie>>
```

No `TmdbService::new(...).search(...)` indirection. Stateful components
(`TorrentManager`, `LibraryIndex`, `TmdbCache`) remain structs held by
`AppContext`; the services that use them are still free functions.

### D8. What stays out of core

**`blowup-tauri` only**:
- `player/` — libmpv FFI, CAOpenGLLayer, Win32 GL child window
- native window management (player window, waveform window, subtitle viewer)
- file dialogs, system tray, menus
- thin command wrappers that call `blowup_core::<domain>::service::*`

**`blowup-server` only**:
- HLS / fMP4 streaming endpoints for iOS playback
- auth, tokens, rate limiting, LAN-bind gating
- static file serving for media streams

**Everything else** (torrent, subtitle, tmdb, library, entries, export,
media, audio, cross-domain workflows) lives in `blowup-core`.

### D9. Server embedded in Tauri process

At desktop runtime, `blowup-tauri` spawns `blowup-server`'s axum router
in-process bound to `127.0.0.1`. The same HTTP code serves the LAN-side
iPad. For headless NAS / Mac-mini deployment, `blowup-server` is also a
standalone binary. Access control (LAN-bind toggle + token) is
server-only and off by default.

### D10. Config is local to each backend — path injected

Current `Config` mixes what will eventually become:
- **Backend config** — TMDB key, library root, tool paths, download settings
- **Client config** — (future) server URL, token, UI theme

YAGNI: keep the single `Config` shape until iOS actually needs a "which
server do I talk to" setting. When that happens, split into `BackendConfig`
and `ClientConfig`.

Path resolution moves out of core into adapters. `core::config::service`
takes a `&Path`:

- Tauri: `app.path().app_data_dir()?.join("config.toml")`
- Server: `$BLOWUP_CONFIG_DIR` env, or `~/.config/blowup-server/config.toml`
- iOS app: SwiftUI side uses `UserDefaults` — not part of core

### D11. ffmpeg → static libav binding (deferred to the final step)

Current: shells out to system `ffmpeg` / `ffprobe` via `FfmpegTool`.

Final target: replace the CLI wrapper with the `ffmpeg-next` Rust bindings
linked against a **statically compiled, heavily-trimmed, LGPL** ffmpeg.

Macro switches (decided):
- `--enable-static --disable-shared`
- **LGPL only** (no `--enable-gpl`) — preserves distribution options
- no network protocols, no hwaccel, no CLI programs, no avdevice, no
  avfilter, no swscale, no postproc, no third-party codec libs
- blowup never decodes or encodes video → all video encoders and most
  video decoders stay off
- blowup never uses filter graphs → `--disable-avfilter/swscale/postproc`

Draft configure script lives in the "Deferred decisions" section at the
end of this doc.

Build infrastructure strategy: pre-build per target, store artifacts in a
separate `blowup-ffmpeg-prebuilt` repo or GitHub Actions cache (decided at
step 7), `FFMPEG_DIR` env drives `ffmpeg-next`'s `build.rs`. Windows is
the hardest platform — lean toward cross-compile from Linux with
mingw-w64.

This step is deferred to the very end because it is orthogonal to the
structural refactor and carries the largest build-system risk.

---

## Migration steps

Each step keeps the desktop app buildable and runnable on its own.

### Step 1 — Workspace skeleton (this step)

- move `src-tauri/` → `crates/tauri/`
- add empty `crates/core/` and `crates/server/` placeholder crates
- update root `Cargo.toml` workspace `members`
- update `tauri.conf.json` `frontendDist` relative path
- update `justfile` recipe paths
- update `.github/workflows/*.yml` working-directories and mpv setup paths
- update `.gitignore`, `eslint.config.js`, `CLAUDE.md` paths
- rename Cargo package `blowup` → `blowup-tauri` (lib name `blowup_lib`
  stays — frontend, build artifacts, `tauri.conf.json` productName all
  unchanged)
- **do not touch any business code**
- verify `just dev`, `just test`, `just clippy`, `just build` still work

### Step 2 — Extract passive modules to core

Move these files roughly as-is, depending on `blowup_core` from
`blowup-tauri`:

| From (`crates/tauri/src/`)                | To (`crates/core/src/`)                |
| ---------------------------------------- | -------------------------------------- |
| `config.rs`                              | `config/` (take `&Path` for load/save) |
| `db/mod.rs`                              | `infra/db.rs`                          |
| `error.rs`                               | `error.rs` (refactor into `CoreError`) |
| `ffmpeg.rs`                              | `infra/ffmpeg.rs`                      |
| `cache.rs`                               | `infra/cache.rs`                       |
| `common.rs`                              | split: exec helpers → `infra/`, paths → `infra/paths.rs` |
| `library_index.rs`                       | `library/index.rs`                     |
| `subtitle_parser.rs`                     | `subtitle/parser.rs`                   |
| `alass.rs`                               | `subtitle/alass.rs`                    |
| `torrent.rs`                             | `torrent/manager.rs`                   |

Commands stay unchanged. `blowup-tauri` imports via `use blowup_core::...`.

### Step 3 — Sink command business logic into core services

For each file in `crates/tauri/src/commands/`:

1. Move the real work (DB queries, HTTP calls, file IO) into
   `crates/core/src/<domain>/service.rs`.
2. Reduce the Tauri command to a 3–5 line thin wrapper.
3. One command (or closely-related group) per commit.

Events: add `core::infra::events::EventBus`, make services publish into
it, add a task in `blowup-tauri` that subscribes and re-emits as Tauri
events. Frontend event names are unchanged.

Done when `crates/tauri/src/commands/` contains nothing but thin wrappers.

### Step 4 — Add `blowup-server` crate ✅

Delivered across batches L–O on branch `refactor/workspace-core-server`.

**Crate layout (`crates/server/src/`)**:

```
main.rs           # standalone bootstrap (BLOWUP_DATA_DIR + BLOWUP_SERVER_BIND)
lib.rs            # build_router(state), serve(addr, state)
state.rs          # AppState
error.rs          # ApiError + IntoResponse
routes/
  health.rs       # GET /health
  config.rs       # GET/POST /config, GET /config/cache-path
  search.rs       # POST /search/yify
  tmdb.rs         # /tmdb/{search,discover,genres,credits/*,credits/enrich}
  media.rs        # GET /media/probe, probe-detail, POST /probe-and-cache
  audio.rs        # GET /audio/streams, POST /audio/extract
  tracker.rs      # /tracker/{status,refresh,user}
  subtitle.rs     # 9 routes (streams/parse/search/download/fetch/align/
                  #           align-to-audio/extract/shift)
  entries.rs      # 13 routes (CRUD + tags + relations + graph)
  library.rs      # 15 routes (items/assets/stats + index ops)
  downloads.rs    # 7 routes (list/start/pause/resume/delete/redo/files)
  export.rs       # 9 routes (local + S3 export/import + s3/test)
  events.rs       # GET /events — SSE stream of DomainEvent
```

All routes mounted under `/api/v1`. Total: 53 endpoints.

**AppState** holds shared handles:

```rust
#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub library_index: Arc<LibraryIndex>,
    pub tracker: Arc<TrackerManager>,
    pub torrent: Arc<tokio::sync::OnceCell<TorrentManager>>,
    pub http: reqwest::Client,
    pub events: EventBus,
}
```

- `SqlitePool` is already cheap-clone (internal `Arc<PoolInner>`)
- `TorrentManager` is gated via `OnceCell` because it's built
  asynchronously; handlers that need it call `state.torrent()?` and
  return a 503-style "still initializing" error if it hasn't landed
  yet
- `EventBus` wraps `tokio::sync::broadcast::Sender<DomainEvent>`

**Error mapping**: `core::*::service` functions mostly return
`Result<T, String>`. The server wraps them in `ApiError::{NotFound,
BadRequest, Internal}` via a heuristic `From<String>` impl and
`IntoResponse` produces a `{ "error": "…" }` JSON body with the
right HTTP status.

**SSE endpoint** (`GET /api/v1/events`) subscribes to the shared
`EventBus`, emits an initial `hello` event, then streams each
`DomainEvent` as `event: downloads:changed` + JSON payload. 15s
keepalives hold the connection open through idle proxies.

**Standalone bootstrap**: `main.rs` reads `BLOWUP_DATA_DIR` (default:
`$DATA_DIR/blowup-server`), initializes config/db/library_index/
tracker/torrent synchronously, constructs an `AppState`, and calls
`axum::serve` on `BLOWUP_SERVER_BIND` (default
`127.0.0.1:17690`). Run with:

```bash
cargo run -p blowup-server
```

**CORS** is permissive for now (`CorsLayer::very_permissive()`).
Auth + LAN-bind gating remain open questions for later.

### Step 5 — Embed server in tauri-app ✅

Delivered in batch P. The desktop app now spawns `blowup_server::serve`
in-process after the async `TorrentManager::new` resolves.

**Shared state**: `blowup-tauri`'s setup() builds the same handles the
standalone server would, registers each one as Tauri managed state,
then constructs an `AppState` from clones/Arcs of the same values and
hands it to the embedded `blowup_server::serve("127.0.0.1:17690", …)`.
Both the Tauri IPC bridge and the axum router see the same
`SqlitePool`, `Arc<LibraryIndex>`, `TorrentManager` (via `OnceCell`),
`TrackerManager`, and `EventBus`.

The `LibraryIndex` Tauri state type changed from `LibraryIndex` to
`Arc<LibraryIndex>`; deref coercion keeps every wrapper body
unchanged — only the signature moved.

**Unified event path**: every Tauri wrapper that used to call
`app.emit("xxx:changed", ())` now calls
`events.publish(DomainEvent::Xxx)`. A single listener task spawned in
`lib.rs` subscribes to the bus and forwards each event via
`app.emit(event.as_str(), ())`, so the desktop frontend keeps
receiving the exact same string identifiers without changes. The
embedded server's SSE endpoint subscribes to the **same bus**, so
mutations from the desktop side propagate to LAN-side iOS clients.

**Failure modes**:
- If the embedded server fails to bind `127.0.0.1:17690` (port in
  use), Tauri logs a warning and keeps running — LAN access is
  disabled, desktop still works.
- If `TorrentManager::new` fails, the `OnceCell` stays empty; both
  Tauri download commands and the server's download routes return
  the same "still initializing" error.

**Port**: hard-coded `127.0.0.1:17690` for now. To be promoted to a
user setting in a later round.

### Step 6 — iOS client

Separate Xcode project at `ios/`. SwiftUI, VLCKit for playback,
`URLSession` for API calls against `blowup-server`.

### Step 7 — Replace CLI ffmpeg with static libav

See D11 and the "Deferred decisions" section.

---

## Workflows (cross-domain orchestration)

Workflows are the only place where multiple domain services can be called
in sequence. Initial list:

1. **`download_complete_to_library`** — on torrent completion:
   move files into `{library_root}/{director}/{tmdb_id}/`, write the
   `library_index` entry, trigger TMDB enrichment, emit `LibraryChanged`
   and `DownloadsChanged`.

Further workflows will be added as the pattern proves out.

---

## Deferred decisions (revisit at the relevant step)

### Audio waveform — where does decoding happen?

- **Option A**: wavesurfer.js decodes on the frontend. Server streams the
  audio file as-is.
- **Option B**: core decodes to PCM, computes amplitude data, sends JSON
  to the frontend. Wavesurfer consumes a prebuilt waveform.

**Leaning B** — avoids making iOS decode a TrueHD 5.1 track. Affects the
ffmpeg trim config (audio decoder whitelist + `swresample`).

### ffmpeg trimmed `./configure` draft

```bash
./configure \
  --enable-static --disable-shared --enable-pic \
  --disable-everything \
  --disable-programs --disable-doc --disable-debug \
  --disable-network --disable-autodetect \
  --disable-avdevice --disable-avfilter \
  --disable-swscale --disable-postproc \
  --disable-iconv --disable-bzlib --disable-lzma \
  \
  --enable-demuxer=matroska,mov,mp4,mpegts,mpegps,avi,flv \
  --enable-demuxer=mp3,flac,ogg,aac,ac3,wav \
  --enable-demuxer=srt,ass,webvtt,subviewer \
  \
  --enable-muxer=matroska,mov,mp4,mpegts,fmp4,hls \
  --enable-muxer=srt,ass,webvtt \
  \
  --enable-parser=h264,hevc,av1,vp9 \
  --enable-parser=aac,ac3,opus,flac,mpegaudio \
  \
  --enable-bsf=h264_mp4toannexb,hevc_mp4toannexb \
  --enable-bsf=aac_adtstoasc,extract_extradata \
  \
  --enable-protocol=file,pipe \
  \
  --enable-decoder=subrip,ass,ssa,movtext,webvtt \
  --enable-decoder=hdmv_pgs_subtitle,dvd_subtitle \
  --enable-encoder=subrip,ass,webvtt \
  \
  # Only if waveform Option B is chosen:
  --enable-decoder=aac,ac3,eac3,mp3,opus,flac,truehd,dts,pcm_s16le,pcm_s24le,pcm_f32le \
  --enable-swresample
```

### Prebuilt ffmpeg artifact storage

Lean toward a separate `blowup-ffmpeg-prebuilt` repo with per-target `.a`
release artifacts, referenced by tag from `build.rs`. Alternative is
GitHub Actions cache. Decide at step 7.

### Windows ffmpeg build approach

Lean toward cross-compile from Linux with `mingw-w64` (CI-friendly).
Alternative is native MSYS2. Decide at step 7.

---

## Open questions

- **Server auth**: currently CORS is permissive and there's no token.
  Acceptable while both the server and its client live on localhost,
  but the moment the user wants to reach the desktop from an iPad on
  the same Wi-Fi, we need a bind toggle (`127.0.0.1` → `0.0.0.0`) and
  a token. Revisit at step 6.
- **HLS segment lifetime**: no streaming routes yet. When step 7 or
  an earlier iOS-streaming push lands, decide between a tempdir with
  idle cleanup or per-video persistent segments.
- **In-process server port**: hard-coded `127.0.0.1:17690`. Promote
  to `Config.server.port` + `Config.server.bind` when the iOS client
  starts needing to discover it.
- **Downloads progress on standalone server**: `blowup-tauri`'s
  embedded monitor task drives DB updates + event publishes. The
  standalone `blowup-server` binary currently does not spawn its own
  monitor — iOS clients against a headless server see downloads
  transition to `downloading` but progress bytes stay at zero until
  they poll again. A short-term fix is to spawn an equivalent monitor
  inside the server's `start_download` / `resume_download` / `redownload`
  handlers.
