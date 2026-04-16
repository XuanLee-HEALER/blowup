# CLAUDE.md

## Project Overview

**blowup** v2.1.1 — A Tauri v2 desktop app for the Chinese film-watching pipeline: TMDB discovery, torrent search & download, subtitle management, personal film knowledge base, and media playback. v2.1 adds a local MCP bridge so Claude Code / Cursor / Cline can read and write the knowledge base over a Unix-domain socket.

Named after Michelangelo Antonioni's 1966 film *Blow-Up*.

GitHub: https://github.com/XuanLee-HEALER/blowup

## Architecture

**4-crate** Rust workspace + React 19 frontend. The single-`src-tauri` layout was split in 2026-04; the MCP crate was added in 2026-04 for the v2.1 Skill Bridge — see `docs/REFACTOR.md`.

```
crates/
├── core/                        # blowup-core — pure business logic, zero Tauri/HTTP coupling
│   ├── src/
│   │   ├── lib.rs               # module roots
│   │   ├── context.rs           # AppContext (canonical shared state, see below)
│   │   ├── error.rs             # typed errors + status::not_found / bad_request prefix helpers
│   │   ├── config/              # TOML config load/save (path injected by adapter)
│   │   ├── infra/               # cross-domain plumbing
│   │   │   ├── cache.rs         # TMDB credits LRU cache (parking_lot Mutex)
│   │   │   ├── common.rs        # exec_command, find_command_path, normalize_director_name
│   │   │   ├── db/              # SqlitePool init + sqlx::migrate!("../../crates/core/migrations")
│   │   │   ├── events.rs        # EventBus (tokio::sync::broadcast) + DomainEvent
│   │   │   ├── ffmpeg.rs        # FfmpegTool wrapper (ffmpeg/ffprobe)
│   │   │   └── paths.rs         # is_safe_relative_path, is_within_root
│   │   ├── library/             # Film library — owner of the on-disk tree
│   │   │   ├── index.rs         # LibraryIndex + .index.json persistence
│   │   │   └── items.rs         # SQLite library_items / library_assets CRUD
│   │   ├── tmdb/                # Stateless TMDB API + cached enrichment
│   │   ├── torrent/             # YTS search (search.rs) + librqbit manager + downloads table
│   │   ├── subtitle/            # SRT/ASS parser + alass-core align + OpenSubtitles/ASSRT
│   │   ├── media/               # ffprobe wrapper + cache writeback
│   │   ├── audio/               # ffmpeg-based stream extract + waveform peaks
│   │   ├── entries/             # Knowledge base entries + tags + relations + graph view
│   │   ├── export/              # KB / config export-import (local JSON + S3)
│   │   ├── tasks/               # In-memory long-task registry (subtitle align, ...)
│   │   └── workflows/           # Cross-domain orchestration (see workflows section)
│   └── migrations/              # SQLite schema (1..5) — owned by core, NOT by tauri
│
├── server/                      # blowup-server — axum HTTP wrapper around blowup-core
│   ├── src/
│   │   ├── lib.rs               # build_router → Bearer middleware on every /api/v1 route
│   │   ├── main.rs              # standalone binary (BLOWUP_DATA_DIR / _BIND / _TOKEN env)
│   │   ├── auth.rs              # require_bearer middleware + generate_random_token helper
│   │   ├── path_guard.rs        # re-export of core::infra::paths
│   │   ├── error.rs             # ApiError ← strip_prefix on status::* tags
│   │   ├── state.rs             # `pub use blowup_core::AppContext as AppState`
│   │   ├── serve_unix.rs        # v2.1 — serve the router over AF_UNIX for Skill Bridge
│   │   └── routes/              # one file per domain (health/config/search/tmdb/media/...)
│   └── tests/
│       ├── smoke.rs             # 11 router smoke tests (auth + read-empty + 404)
│       └── serve_unix.rs        # Unix-socket transport smoke test
│
├── mcp/                         # blowup-mcp — v2.1 MCP server (bundled child binary)
│   ├── src/
│   │   ├── lib.rs               # crate root
│   │   ├── main.rs              # stdio MCP service — launched by Claude Code / Cursor / Cline
│   │   ├── service.rs           # rmcp tool definitions (list_entries / create_entry /
│   │   │                          update_wiki / add_tag / add_relation / list_relation_types)
│   │   ├── client.rs            # minimal hyperlocal (AF_UNIX) JSON client used by service
│   │   ├── socket.rs            # socket path resolution; Windows = "unsupported" sentinel
│   │   └── error.rs             # 4-layer McpError model with [FATAL] prefix convention
│   └── tests/
│       └── smoke.rs             # cross-process stdio + Unix socket smoke test
│
└── tauri/                       # blowup-tauri — desktop adapter (mpv player + Tauri commands)
    ├── src/
    │   ├── lib.rs               # tauri::Builder setup, command registration, embedded server
    │   ├── main.rs              # binary entry
    │   ├── common.rs            # shellexpand helpers used by tauri commands only
    │   ├── skill_bridge/        # v2.1 — SkillBridgeState (parking_lot-guarded socket listener)
    │   ├── player/              # Embedded mpv (CAOpenGLLayer + WKWebView, parking_lot statics)
    │   │   ├── mod.rs           # MpvPlayer lifecycle + event loop
    │   │   ├── ffi.rs           # mpv C API FFI bindings
    │   │   ├── native.rs        # Rust ↔ ObjC/C bridge (macOS/Windows)
    │   │   └── commands.rs      # Tauri player commands (play, seek, sub-add, ...)
    │   └── commands/            # Thin wrappers around blowup_core::* + DomainEvent publish
    │       ├── audio/config/download/export/media/search/subtitle/tasks/tracker.rs
    │       ├── splash.rs        # v2.1 — close_splashscreen (close splash + show main)
    │       ├── skill.rs         # v2.1 — skill_bridge_{status,start,stop,install_to_claude_code}
    │       ├── tmdb/{search,credits,enrichment}.rs
    │       └── library/{entries,graph,items}.rs
    ├── resources/               # Bundled via tauri.conf.json resources
    │   ├── blowup-mcp           # The blowup-mcp binary, synced in dev by `just` recipe
    │   └── skills/blowup-wiki-writer/SKILL.md  # Claude Code skill prompt
    └── native/
        ├── metal_layer.{h,m}    # macOS: CAOpenGLLayer + NSView for mpv rendering
        └── win_gl_layer.{h,c}   # Windows: Win32 OpenGL child window

src/                             # React 19 frontend (TypeScript + Vite + Mantine v9)
├── main.tsx + App.tsx           # Main window entry + three-space shell
├── spaces/                      # v2.1 three-space layout (sidebar spaces)
│   ├── LibrarySpace.tsx         #   Film library + Darkroom (context panel tabs)
│   ├── DiscoverSpace.tsx        #   Search + Download
│   ├── KnowledgeSpace.tsx       #   Wiki + Graph
│   └── SettingsOverlay.tsx      #   Settings modal (incl. Skill Bridge section)
├── splash/ + splash-main.tsx    # v2.1 standalone splashscreen window (animejs)
│   ├── Splash.tsx               #   SVG film camera + particles (animejs timeline)
│   ├── Splash.css               #   .splash-root fade-out transition
│   └── SplashRoot.tsx           #   onComplete → 350ms fade → invoke("close_splashscreen")
├── components/
│   ├── contextpanel/            #   v2.1 right-side context panel tabs
│   │   ├── LibraryDetailTab.tsx
│   │   └── LibraryDarkroomTab.tsx
│   ├── FilmDetailPanel.tsx      # TMDB film detail + YTS search modal
│   ├── WikiDetailView.tsx       # Markdown editor + outline + preview
│   ├── MusicPlayer.tsx          # Background music player
│   └── ui/                      # Button / Chip / NavItem / TextInput
├── pages/                       # Search / Wiki / Graph / Library / Download / Darkroom / Settings
├── lib/
│   ├── tauri.ts                 # All Tauri invoke wrappers + TS types (incl. skillBridge.*)
│   ├── useBackendEvent.ts       # Event hook + BackendEvent constants
│   ├── theme.ts                 # Mantine v9 theme (NO custom color palette — see note below)
│   ├── mountReactRoot.tsx       # Shared StrictMode + MantineProvider wrapper
│   └── format.ts                # Shared formatters
├── Player.tsx + player-main.tsx              # Embedded player UI + window entry
├── SubtitleViewer.tsx + subtitle-viewer-main.tsx   # Subtitle viewer window
└── Waveform.tsx + waveform-main.tsx                # Audio waveform (wavesurfer.js)

index.html + splash.html + player.html + subtitle-viewer.html + waveform.html
# 5 HTML entries — each is a separate Tauri window registered in vite.config.ts.
```

## Two runtime modes, one core

The same `blowup-core` library backs two binaries:

| Mode | Binary | Frontend | Started by |
|------|--------|----------|------------|
| Desktop | `blowup-tauri` | Tauri WebView + React via Tauri IPC `invoke()` | `just dev` / `just build` |
| Headless | `blowup-server` | LAN clients (future iOS, curl) via HTTP/SSE | `just dev-server` / `just build-server` |

Desktop mode also boots an in-process `blowup-server` on `127.0.0.1:17690` so a LAN iPad can share the same DB + library + torrent session as the host machine.

## Development Commands

`just` is the canonical task runner — see `justfile`:

```bash
just              # List all recipes
just dev          # Desktop dev (Tauri + Vite, hot reload)
just dev-server   # Standalone HTTP server (no WebView)
just check        # lint + typecheck + clippy + fmt-check + test
just test         # Workspace tests
just build        # Tauri installer
just build-server # Standalone server release binary
```

Frontend uses **bun** (`bun install`, `bun run`, `bunx`).

## Code Style & Conventions

### Rust
- **`AppContext`** in `blowup_core::context` is the canonical bundle of shared state (db, library_index, tracker, torrent OnceCell, http, events, tasks, auth_token). Both adapters construct it; the server uses it directly as its `AppState`, the Tauri adapter currently still also `handle.manage()`s individual fields for legacy `State<T>` command signatures.
- **Cross-domain orchestration** lives in `crates/core/src/workflows/`. Single-domain modules should not reach into other domains. `LibraryIndex` is the documented exception — it's treated as an infra-level type since several domains write into it (see `crates/core/src/library/mod.rs` head comment).
- **Error status convention** — services return `Result<T, String>`. To map an error to a non-500 HTTP status, build the string via `core::error::status::not_found(msg)` / `bad_request(msg)`. The axum adapter `strip_prefix`-matches; the Tauri adapter forwards the prefix through to the frontend.
- **No `unwrap()` in non-test code.** Use `parking_lot::{Mutex, RwLock}` (no poisoning → no `unwrap` needed). Tokio async locks (`tokio::sync::*`) are still allowed.
- **Tauri v2 pool access**: `pool.inner()`, NOT `&**pool`.
- **Runtime SQL**: `sqlx::query_as::<_, T>("SQL")` — no compile-time DATABASE_URL.
- **`#[derive(sqlx::FromRow)]`** only on flat structs matching DB columns.
- **Tauri command errors** stringify with `.map_err(|e| e.to_string())?`.
- **Tests** use in-memory SQLite (`SqlitePool::connect(":memory:")` + `sqlx::migrate!("../core/migrations")`).
- **Commit messages** follow conventional commits (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`).

### TypeScript / React
- All UI strings in Chinese.
- CSS custom properties (`var(--color-*)`) — NOT Tailwind classes.
- Inline styles, no CSS modules.
- `useEffect(..., [deps])` for data loading — never `useState(() => { api })`.
- Shared formatters in `src/lib/format.ts`.
- Wiki HTML sanitized with DOMPurify.
- **Mantine colors are a minefield.** The theme in `src/lib/theme.ts` does NOT register a custom palette — `accent` / `success` / `danger` / `warning` are NOT Mantine color names, they are only CSS variables. Never pass them to a Mantine component as `color="accent"` — it silently falls back to primary (blue) or renders transparent (Progress fill). To color Mantine components with app-theme colors, use `styles={{ root: { backgroundColor: "var(--color-accent)" } }}` or equivalent slot override. Bug class that already bit us twice: `<Badge color="accent">` and `<Progress color="accent">`.
- **Splash window** is a separate Tauri webview (`splash.html` → `src/splash-main.tsx`). It mounts WITHOUT `MantineProvider` to keep the bundle minimal. The main window boots with `visible:false` and is only shown when the splash animation finishes and invokes `close_splashscreen`. Do NOT call `open_devtools()` on a hidden main window in `setup()` — it races with the later `show()` and crashes the app on macOS (v2.1 regression fix).

## Data Architecture: Knowledge Base vs Film Library

These are **two independent systems**. Never conflate them.

### Knowledge Base (知识库) — SQLite

Unified entry model: everything is an **entry** (影人、电影、流派、概念... 都是条目).

```sql
entries:     id, name, wiki, created_at, updated_at
entry_tags:  entry_id, tag (PK: entry_id, tag)
relations:   id, from_id, to_id, relation_type
```

- No type field — the distinction between "影人" and "电影" is entirely by user-applied tags.
- Relation types are open-ended (user-created strings, not constrained).
- Wiki content is Markdown stored on the entry row.
- Pages: `Wiki.tsx` (list + detail), `Graph.tsx` (D3 force graph).

### Film Library (电影库) — File system + JSON index

- Storage: `{library.root_dir}/{director}/{tmdb_id}/` directories.
- Index: `library_index.json` — in-memory `IndexEntry` array, persisted to JSON.
- Each `IndexEntry` carries: `tmdb_id`, `title`, `director`, `year`, `genres`, `path`, `files[]`, plus cached TMDB enrichment (poster, overview, rating, credits) and per-file `media_info` + `subtitle_configs`.
- Pages: `Library.tsx` (director tree + detail panel), `Darkroom.tsx` (subtitle/media tools).
- **Film detail data comes from the file index, NOT from SQLite.**

### The only connection
A wiki entry can hyperlink to a Library detail page if the same film exists in both. That's it.

## Frontend-Backend Interaction: Event-Driven Refresh

Notify + refetch — backend mutations emit domain events, frontend listens and re-fetches.

5 domain events (no payload, fire-and-forget):

| Event | Emitters | Listeners |
|-------|----------|-----------|
| `downloads:changed` | `core::workflows::download_monitor` (every 2s) + downloads CRUD | `Download.tsx` |
| `library:changed`   | download monitor on complete, `library/items.rs`, `tmdb/enrichment.rs` | `Library.tsx`, `Darkroom.tsx` |
| `entries:changed`   | `entries::service` (all writes), `export::service` (import) | `Wiki.tsx`, `Graph.tsx` |
| `config:changed`    | `config::save_config`, `export::service` (import) | `App.tsx`, `Search.tsx` |
| `tasks:changed`     | `core::workflows::subtitle_align` (start / complete / fail) | task store in frontend |

**Backend**: `events.publish(DomainEvent::XXX)` on the shared `EventBus` — Tauri forwards each to `app.emit(name, ())`, server pushes via `/api/v1/events` SSE.

**Frontend**: `useBackendEvent(BackendEvent.XXX, refresh)` hook in `src/lib/useBackendEvent.ts`. The `BackendEvent` TS enum prevents typo-induced silent failures.

## Workflows (cross-domain orchestration)

Anything that touches more than one domain lives in `crates/core/src/workflows/`. Single-domain modules call it; it doesn't get called the other way.

| Workflow | What it does |
|----------|-------------|
| `subtitle_align::run_subtitle_align_to_audio` | Insert task record, spawn alass-core align, update registry on done/fail |
| `subtitle_align::run_subtitle_align_to_video` | Same but extracts audio stream first |
| `download_monitor::spawn` | Poll `librqbit` handle every 2s, mark progress, on complete: extract subs → scan files → add to library index |

Both Tauri commands and server routes call into the same `workflows::*` functions, so desktop and standalone server share completion semantics.

## blowup-server: HTTP API

- All routes live under `/api/v1/`, mounted in `crates/server/src/lib.rs::build_router`.
- **Every** route requires `Authorization: Bearer <token>` via the `auth::require_bearer` middleware.
- **No CORS layer** — browsers cannot reach the API even if they learn the token (preflight is blocked). Native clients (iOS, curl) are unaffected.
- Token resolution: `$BLOWUP_SERVER_TOKEN` if set, otherwise generated randomly per session and logged at WARN level.
- SSE endpoint at `/api/v1/events` mirrors the in-process `EventBus`.
- **AF_UNIX mode** (v2.1): `serve_unix.rs` exposes the same router over a local Unix domain socket with the bearer middleware *removed* (`build_router_trusted`). Filesystem perms (0600 on socket, 0700 on parent dir) replace the token — which is exactly what the Skill Bridge uses.
- Smoke-tested in `crates/server/tests/smoke.rs` (auth 401/200, unknown route 404, fresh-install reads, missing-resource 404) and `tests/serve_unix.rs`.

## Skill Bridge (v2.1)

Lets a local AI client (Claude Code / Cursor / Cline / Zed) read and write the knowledge base via MCP over a Unix-domain socket. Three moving parts:

1. **`crates/mcp/`** — the `blowup-mcp` binary, shipped inside the Tauri bundle. It is an stdio MCP server (rmcp) that the AI client spawns on demand. Its tools (list_entries, get_entry, create_entry, update_wiki, add_tag, add_relation, list_relation_types, list_all_tags) all forward to the desktop app via the hyperlocal client in `crates/mcp/src/client.rs`.
2. **`crates/server/src/serve_unix.rs`** — the Tauri process listens on an AF_UNIX socket using `build_router_trusted` (no bearer gate; the 0600 socket + 0700 parent dir is the trust boundary).
3. **`crates/tauri/src/skill_bridge/` + `commands/skill.rs`** — the Tauri-side state machine: `skill_bridge_start` creates/cleans the socket and spawns the listener; `skill_bridge_stop` tears it down; `skill_bridge_install_to_claude_code` copies the bundled `blowup-mcp` binary + `SKILL.md` into the user's Claude Code config dir and runs `claude mcp add` if the CLI is available.

Rules:
- Skill Bridge is **unsupported on Windows** — `crates/mcp/src/socket.rs::default_socket_path()` returns a sentinel path and the Tauri command reports `supported: false`. Frontend hides the section.
- Errors from the MCP service use a 4-layer model (`crates/mcp/src/error.rs`) and prefix fatal classes with `[FATAL]` so the skill prompt can tell retryable from non-retryable.
- The window-close handler (`on_window_event` in `crates/tauri/src/lib.rs`) calls `SkillBridgeState::shutdown_blocking()` on the main window only — closing the player popout or a subtitle viewer must NOT tear down the bridge.
- The Tauri app's `resource_dir()` looks different in dev vs release. `justfile::_sync-dev-resources` copies `crates/tauri/resources/skills` into `target/debug/` so dev runs see the same layout as a bundled app; the `blowup-mcp` binary is already at `target/debug/blowup-mcp` as a workspace member. Don't skip this recipe.

## Key Patterns

| Pattern | Location | Note |
|---------|----------|------|
| Tauri invoke wrappers | `src/lib/tauri.ts` | All backend calls go through typed wrappers |
| Event-driven refresh | `src/lib/useBackendEvent.ts` | Backend emits → frontend refetches → React re-renders |
| Entry + tags query | `crates/core/src/entries/service.rs` | LEFT JOIN + GROUP_CONCAT for tag aggregation |
| Background downloads | `crates/core/src/workflows/download_monitor.rs` | Shared by both Tauri and server start_download |
| File probing | `crates/core/src/media/service.rs` + `library/items.rs` | ffprobe JSON → structured `FileMediaInfo`, cached in index |
| TMDB lazy enrichment | `crates/core/src/tmdb/service.rs` | `enrich_index_entry` hits TMDB + downloads poster, caches in index |
| Multi-subtitle overlay | `crates/core/src/subtitle/parser.rs` | Parse SRT/ASS → merge to single ASS with per-source style/position; hash-based cache |
| Auto subtitle extraction | `crates/core/src/workflows/download_monitor.rs` | Extracts embedded subs to `.srt` after completion |
| Export/Import | `crates/core/src/export/service.rs` | entries + entry_tags + relations → JSON / S3 |
| Path safety | `crates/core/src/infra/paths.rs` | `is_safe_relative_path`, `is_within_root` — applied wherever user-owned strings join the library root |
| Long-running tasks | `crates/core/src/tasks/registry.rs` + `workflows/subtitle_align.rs` | Generation-guarded slot updates so dismiss+restart races don't clobber state |
| Splash window | `src/splash/` + `crates/tauri/src/commands/splash.rs` | Independent Tauri window 560×340 transparent; animejs timeline onComplete → 350ms CSS fade → invoke close_splashscreen; main window boots `visible:false`, shown only after splash closes |
| Single instance | `tauri_plugin_single_instance` in `crates/tauri/src/lib.rs::run` | Re-launching blowup focuses the existing main window instead of creating a second process |
| Skill Bridge | `crates/mcp/` + `crates/server/src/serve_unix.rs` + `crates/tauri/src/skill_bridge/` + `commands/skill.rs` | See the "Skill Bridge" section above |

## External Service Quirks

- **YIFY**: Official API at `movies-api.accel.li/api/v2/` (the old `yts.torrentbay.st` returns HTML instead of JSON).
- **OpenSubtitles**: REST API at `api.opensubtitles.com/api/v1`. Requires `Api-Key` header. Search is free; download needs JWT login (optional, ~5/day without auth). Old XML-RPC (`api.opensubtitles.org`) is deprecated.
- **TMDB**: free API key from themoviedb.org.

## Runtime Dependencies

| Tool | Used by | Default config key |
|------|---------|-------------------|
| `ffmpeg` + `ffprobe` | Subtitle extraction, media probe, library scan, audio peaks | `tools.ffmpeg` |

Subtitle alignment uses the bundled `alass-core` crate (no external `alass` binary).

Tool paths are auto-detected on startup (`config::resolve_tool_paths`): if the configured path is invalid, it searches `$PATH` and well-known dirs (`/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`) and writes back to config. This handles macOS GUI apps not inheriting shell `PATH`.

## Config

Path: `{APP_DATA_DIR}/config.toml` (migrated from `~/.config/blowup/config.toml` on first run).

```toml
[tools]
ffmpeg = "ffmpeg"

[download]
max_concurrent = 3
enable_dht = true
persist_session = false

[tmdb]
api_key = ""

[opensubtitles]
api_key = ""

[assrt]
token = ""

[subtitle]
default_lang = "zh"

[library]
root_dir = "~/Movies/blowup"

[music]
enabled = false
mode = "sequential"    # or "random"
playlist = []

[cache]
max_entries = 200

[sync]
endpoint = ""
bucket = ""
access_key = ""
secret_key = ""
```

Server-mode environment variables (do NOT live in config.toml):

| Var | Default | Purpose |
|-----|---------|---------|
| `BLOWUP_DATA_DIR` | `dirs::data_dir()/blowup-server` | Server's app data root |
| `BLOWUP_SERVER_BIND` | `127.0.0.1:17690` | axum bind address |
| `BLOWUP_SERVER_TOKEN` | (random per session) | Bearer token shared with iOS/LAN clients |

## Database

SQLite at `{APP_DATA_DIR}/blowup.db`, schema in `crates/core/migrations/`. Tables: `entries`, `entry_tags`, `relations`, `library_items`, `library_assets`, `downloads`.

App lifecycle:
- On startup, stale `downloading` records are reset to `paused` (crash recovery).
- On clean exit, active downloads are paused before the torrent session shuts down.
- `resume_download` re-adds the torrent from its magnet link if the session was lost.

Migration files are byte-stable — sqlx checksums the entire file content (comments included), so even a comment edit on an existing migration breaks every existing install. Add a new migration instead.
