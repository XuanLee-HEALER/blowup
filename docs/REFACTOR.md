# Workspace Refactor — core / server / tauri

> **Status**: in progress on branch `refactor/workspace-core-server`
> **Goal**: split the single `src-tauri` crate into a `blowup-core` library plus
> two thin adapters (`blowup-server`, `blowup-tauri`) so that a future native
> iOS/iPadOS client can share the same Rust business logic via HTTP.

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

### Step 4 — Add `blowup-server` crate

- `axum` router with REST routes mirroring the Tauri commands
- SSE `/api/v1/events` fed by the broadcast channel
- standalone binary that boots its own `AppContext` from a server-side
  config path
- initially not wired into the desktop binary

### Step 5 — Embed server in tauri-app (optional, after Step 4 is stable)

At startup, `blowup-tauri` spins up an in-process axum router on
`127.0.0.1:<fixed-port>`. The same process serves both Tauri commands and
HTTP for LAN clients.

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

- Server auth default: bind `127.0.0.1` with no auth; require a token
  only when binding to LAN. Revisit at step 4.
- HLS segment lifetime: tempdir + cleanup after N minutes idle, or
  persisted per video? Revisit at step 4.
- In-process server port inside `blowup-tauri`: fixed default
  (e.g. `17690`) vs auto-pick. Fixed is simpler for iOS client
  configuration. Revisit at step 5.
