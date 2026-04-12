# CLAUDE.md

## Project Overview

**blowup** v2.0.5 — A Tauri v2 desktop app for the Chinese film-watching pipeline: TMDB discovery, torrent search & download, subtitle management, personal film knowledge base, and media playback.

Named after Michelangelo Antonioni's 1966 film *Blow-Up*.

GitHub: https://github.com/XuanLee-HEALER/blowup

## Architecture

Dual codebase: Rust backend (Tauri commands + SQLite) and React 19 frontend (TypeScript + Vite).

```
crates/tauri/                 # Rust backend (Tauri v2) — was src-tauri/ before workspace split
├── src/
│   ├── lib.rs               # Tauri app builder, command registration
│   ├── config.rs             # Config struct, TOML ser/de, load_config(), save_config()
│   ├── db/mod.rs             # SQLite pool init (sqlx + migrations)
│   ├── error.rs              # thiserror enums per domain
│   ├── ffmpeg.rs             # FfmpegTool wrapper (ffmpeg/ffprobe)
│   ├── common.rs             # exec_command, find_command_path, normalize_director_name
│   ├── cache.rs              # LRU cache (TMDB credits)
│   ├── library_index.rs      # In-memory IndexEntry index, persisted to JSON
│   ├── subtitle_parser.rs    # SRT/ASS parsing + multi-sub ASS merger + overlay cache
│   ├── alass.rs              # Built-in subtitle alignment (alass-core, no external binary)
│   ├── torrent.rs            # librqbit TorrentManager wrapper
│   ├── player/               # Embedded mpv player (CAOpenGLLayer + WKWebView)
│   │   ├── mod.rs            # MpvPlayer lifecycle, event loop (push model)
│   │   ├── ffi.rs            # mpv C API FFI bindings
│   │   ├── native.rs         # Rust ↔ ObjC/C bridge (macOS/Windows)
│   │   └── commands.rs       # Tauri player commands (play, seek, sub-add, etc.)
│   └── commands/
│       ├── config.rs         # get_config, save_config_cmd
│       ├── search.rs         # YTS torrent search (movies-api.accel.li)
│       ├── download.rs       # Torrent download management (librqbit)
│       ├── tmdb.rs           # TMDB search/discover/credits + index enrichment
│       ├── tracker.rs        # BitTorrent tracker list update
│       ├── subtitle.rs       # OpenSubtitles/ASSRT + ffmpeg extraction + auto-extract
│       ├── audio.rs          # Audio stream extraction + waveform window
│       ├── media.rs          # probe_media_detail, probe_and_cache
│       ├── export.rs         # Knowledge base + config export/import (local + S3)
│       └── library/          # Knowledge base + film library
│           ├── mod.rs        # Shared types (EntrySummary, EntryDetail, LibraryItemSummary, etc.)
│           ├── entries.rs    # Entry CRUD, tags, relations
│           ├── graph.rs      # D3 graph data (entry-relation links)
│           └── items.rs      # Library items + scan + assets + stats + index + delete commands
├── native/
│   ├── metal_layer.h/.m     # macOS: CAOpenGLLayer + NSView for mpv rendering
│   └── win_gl_layer.h/.c    # Windows: Win32 OpenGL child window
├── migrations/
│   ├── 001_initial.sql       # library_items, library_assets
│   ├── 002_downloads.sql     # downloads table (legacy, replaced by 003)
│   ├── 003_download_refactor.sql  # downloads table (librqbit)
│   ├── 004_knowledge_base_v2.sql  # entries, entry_tags, relations (unified KB model)
│   └── 005_downloads_year_genres.sql  # Add year, genres to downloads
└── Cargo.toml

src/                          # React 19 frontend (TypeScript)
├── App.tsx                   # Router + sidebar nav + MusicPlayer
├── lib/
│   ├── tauri.ts              # All Tauri invoke wrappers + TS types
│   ├── useBackendEvent.ts    # Event hook + BackendEvent constants (notify+refetch)
│   ├── format.ts             # Shared formatters (size, duration, bitrate)
│   └── styles.ts             # Shared style constants
├── pages/
│   ├── Search.tsx            # TMDB search/discover with filters
│   ├── Wiki.tsx              # Knowledge base: entry list + detail + tags + relations
│   ├── Graph.tsx             # Knowledge graph: D3 force simulation
│   ├── Library.tsx           # Film library: director tree + detail panel (poster, credits, files)
│   ├── Download.tsx          # Download queue + history + manual add
│   ├── Darkroom.tsx          # 暗房: subtitle tools + media probe (unified)
│   └── Settings.tsx          # Config editor
├── Player.tsx                # Embedded player controls (liquid glass UI)
├── player-main.tsx           # Player window React entry
├── SubtitleViewer.tsx        # Subtitle viewer window
├── Waveform.tsx              # Audio waveform visualizer (wavesurfer.js)
└── components/
    ├── FilmDetailPanel.tsx   # TMDB film detail + YTS search modal
    ├── WikiDetailView.tsx    # Shared markdown editor + outline + preview
    ├── MusicPlayer.tsx       # Background music player
    └── ui/                   # Primitives: Button, Chip, NavItem, TextInput
```

## Development Commands

Uses `just` as task runner (see `justfile` for all recipes):

```bash
just              # Show all available recipes
just dev          # Tauri dev server (frontend + backend hot reload)
just check        # Run lint + typecheck + test
just test         # Rust tests only
just lint         # ESLint
just typecheck    # bunx tsc --noEmit
just build        # Production build (Tauri installer)
just clippy       # Rust clippy
just fmt          # Rust format
```

Frontend uses **bun** as package manager and script runner (`bun install`, `bun run`, `bunx`).

## Code Style & Conventions

### Rust (crates/tauri/)
- Tauri v2 pool access: `pool.inner()` — NOT `&**pool`
- Runtime queries: `sqlx::query_as::<_, T>("SQL")` — no compile-time DATABASE_URL
- `#[derive(sqlx::FromRow)]` only on flat structs matching DB column names
- Errors: `Result<T, String>` for Tauri commands, `.map_err(|e| e.to_string())?`
- Tests: in-memory SQLite `SqlitePool::connect(":memory:")` + `sqlx::migrate!("./migrations")`
- No `unwrap()` in non-test code
- Commit messages: conventional commits (`feat:`, `fix:`, `docs:`, `chore:`)

### TypeScript/React (src/)
- All UI strings in Chinese
- CSS custom properties (`var(--color-*)`) — NOT Tailwind classes
- Inline styles — no CSS modules
- `useEffect(..., [deps])` for data loading — never `useState(() => { api })`
- Shared formatters in `src/lib/format.ts`
- Wiki HTML sanitized with DOMPurify

## Data Architecture: Knowledge Base vs Film Library

These are **two independent systems**. Never conflate them.

### Knowledge Base (知识库) — SQLite

Unified entry model: everything is an **entry** (影人、电影、流派、概念... 都是条目).

Three tables:
```sql
entries:     id, name, wiki, created_at, updated_at
entry_tags:  entry_id, tag (PK: entry_id, tag)
relations:   id, from_id, to_id, relation_type
```

- No type field — the distinction between "影人" and "电影" is entirely by user-applied tags
- Relation types are fully open (user-created strings, not constrained)
- Wiki content is Markdown, stored directly on the entry
- Pages: Wiki.tsx (list + detail), Graph.tsx (D3 force graph)

### Film Library (电影库) — File System

- Storage: `{library.root_dir}/{director}/{tmdb_id}/` directories on disk
- Index: `library_index.json` — in-memory `IndexEntry` array, persisted to JSON
- Each `IndexEntry` contains: `tmdb_id`, `title`, `director`, `year`, `genres`, `path`, `files[]`, plus cached data:
  - **TMDB enrichment** (poster, overview, rating, credits) — lazy-loaded on first view
  - **media_info** — cached ffprobe results per video file
  - **subtitle_configs** — saved display settings (color, font_size, y_position) per subtitle file
- Pages: Library.tsx (director tree + detail panel), Darkroom.tsx (subtitle/media tools)
- **Film detail page data comes from the file index, NOT from SQLite**

### The only connection
If a film mentioned in the knowledge base (e.g. in an entry's wiki) also exists in the film library, a hyperlink can navigate to the corresponding Library detail page. That's it.

## Frontend-Backend Interaction: Event-Driven Refresh

Data flow follows **notify + refetch** pattern — backend emits domain events after mutations, frontend listens and re-fetches via existing invoke wrappers.

4 domain events (no payload, fire-and-forget):

| Event | Emitters | Listeners |
|-------|----------|-----------|
| `downloads:changed` | download.rs (progress/state), monitor (every 2s) | Download.tsx |
| `library:changed` | download monitor (on complete), items.rs, enrichment.rs | Library.tsx, Darkroom.tsx |
| `entries:changed` | entries.rs (all 8 write ops), export.rs (import) | Wiki.tsx, Graph.tsx |
| `config:changed` | config.rs (save), export.rs (import) | App.tsx, Search.tsx |

- **Backend**: `app.emit("event:name", ())` via `tauri::Emitter` — add `app: tauri::AppHandle` param to commands
- **Frontend**: `useBackendEvent(BackendEvent.XXX, refresh)` hook in `src/lib/useBackendEvent.ts`
- Event name constants: `BackendEvent` enum (TS) — prevents typo-induced silent failures
- Download.tsx uses events instead of polling (no `setInterval`)

## Key Patterns

| Pattern | Location | Note |
|---------|----------|------|
| Tauri invoke wrappers | `src/lib/tauri.ts` | All backend calls go through typed wrappers |
| Event-driven refresh | `src/lib/useBackendEvent.ts` | Backend emits → frontend refetches → React re-renders |
| Entry + tags query | `library/entries.rs` | LEFT JOIN + GROUP_CONCAT for tag aggregation |
| Background downloads | `commands/download.rs` | Torrent manager with librqbit |
| File probing | `commands/media.rs` + `library/items.rs` | ffprobe JSON → structured MediaInfo |
| TMDB lazy enrichment | `commands/tmdb.rs` | `enrich_index_entry` fetches TMDB data + poster → cached in index |
| Multi-subtitle overlay | `subtitle_parser.rs` | Parse SRT/ASS → merge to single ASS with per-source style/position; hash-based cache |
| Auto subtitle extraction | `commands/download.rs` | Download monitor extracts embedded subs to .srt after completion |
| Export/Import | `commands/export.rs` | entries + entry_tags + relations → JSON |

## External Service Quirks

- **YIFY**: Official API migrated to `movies-api.accel.li/api/v2/` (old `yts.torrentbay.st` returns HTML instead of JSON)
- **OpenSubtitles**: REST API at `api.opensubtitles.com/api/v1`. Requires Api-Key header. Search is free; download needs JWT login (optional, 5/day without auth). Old XML-RPC (`api.opensubtitles.org`) is deprecated.
- **TMDB**: free API key from themoviedb.org

## Runtime Dependencies

| Tool | Used by | Default config key |
|------|---------|-------------------|
| `ffmpeg` + `ffprobe` | Subtitle extraction, media probe, library scan | `tools.ffmpeg` |

Subtitle alignment uses built-in `alass-core` crate (no external binary needed).

Tool paths are auto-detected on startup (`config::resolve_tool_paths`): if the configured path is invalid, searches PATH and well-known dirs (`/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`) then writes back to config. This handles macOS GUI apps not inheriting shell PATH.

## Config

Path: `{APP_DATA_DIR}/config.toml` (migrated from `~/.config/blowup/config.toml` on first run)

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

[search]
rate_limit_secs = 5

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

## Database

SQLite at `{APP_DATA_DIR}/blowup.db`. Tables: `entries`, `entry_tags`, `relations`, `library_items`, `library_assets`, `downloads`.

App lifecycle: on startup, stale `downloading` records are reset to `paused` (crash recovery). On clean exit, active downloads are paused before torrent session shutdown. `resume_download` re-adds torrent from magnet link if session was lost.
