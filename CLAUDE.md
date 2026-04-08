# CLAUDE.md

## Project Overview

**blowup** v3.0.0 — A Tauri v2 desktop app for the Chinese film-watching pipeline: TMDB discovery, torrent search & download, subtitle management, personal film knowledge base, and media playback.

Named after Michelangelo Antonioni's 1966 film *Blow-Up*.

GitHub: https://github.com/XuanLee-HEALER/blowup

## Architecture

Dual codebase: Rust backend (Tauri commands + SQLite) and React 19 frontend (TypeScript + Vite).

```
src-tauri/                    # Rust backend (Tauri v2)
├── src/
│   ├── lib.rs               # Tauri app builder, command registration
│   ├── config.rs             # Config struct, TOML ser/de, load_config(), save_config()
│   ├── db/mod.rs             # SQLite pool init (sqlx + migrations)
│   ├── error.rs              # thiserror enums per domain
│   ├── ffmpeg.rs             # FfmpegTool wrapper (ffmpeg/ffprobe)
│   ├── common.rs             # exec_command, find_command_path
│   └── commands/
│       ├── config.rs         # get_config, save_config_cmd
│       ├── search.rs         # YTS torrent search (movies-api.accel.li)
│       ├── download.rs       # torrent download management
│       ├── tmdb.rs           # TMDB search/discover/credits
│       ├── tracker.rs        # BitTorrent tracker list update
│       ├── subtitle.rs       # fetch/align/extract/list/shift subtitles
│       ├── media.rs          # probe_media_detail, open_in_player
│       ├── export.rs         # Knowledge base + config export/import (local + S3)
│       └── library/          # Knowledge base + film library
│           ├── mod.rs        # Shared types (EntrySummary, EntryDetail, LibraryItemSummary, etc.)
│           ├── entries.rs    # Entry CRUD, tags, relations (12 commands)
│           ├── graph.rs      # D3 graph data (entry-relation links)
│           └── items.rs      # Library items + scan + assets + stats + index commands
├── migrations/
│   ├── 001_initial.sql       # library_items, library_assets (+ legacy tables dropped by 004)
│   ├── 002_downloads.sql     # downloads table (legacy, replaced by 003)
│   ├── 003_download_refactor.sql  # downloads table (librqbit)
│   └── 004_knowledge_base_v2.sql  # entries, entry_tags, relations (unified KB model)
└── Cargo.toml

src/                          # React 19 frontend (TypeScript)
├── App.tsx                   # Router + sidebar nav + MusicPlayer
├── lib/
│   ├── tauri.ts              # All Tauri invoke wrappers + TS types
│   └── format.ts             # Shared formatters (size, duration, bitrate)
├── pages/
│   ├── Search.tsx            # TMDB search/discover with filters
│   ├── Wiki.tsx              # Knowledge base: entry list + detail + tags + relations
│   ├── Graph.tsx             # Knowledge graph: D3 force simulation
│   ├── Library.tsx           # Film grid + file linking + stats + scan
│   ├── Download.tsx          # Download queue + history + manual add
│   ├── Subtitle.tsx          # Subtitle tools (fetch/align/extract/shift)
│   ├── Media.tsx             # Media probe + player launch
│   └── Settings.tsx          # Config editor
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

### Rust (src-tauri/)
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
- Each `IndexEntry` contains: `tmdb_id`, `title`, `director`, `year`, `genres`, `path`, `files[]`, and **enriched TMDB data** (poster, overview, cast) cached in the index
- Enriched data is **lazy-loaded**: if fields are empty on first view, fetch from TMDB API and cache in the index
- Pages: Library.tsx (director tree + detail panel)
- **Film detail page data comes from the file index, NOT from SQLite**

### The only connection
If a film mentioned in the knowledge base (e.g. in an entry's wiki) also exists in the film library, a hyperlink can navigate to the corresponding Library detail page. That's it.

## Key Patterns

| Pattern | Location | Note |
|---------|----------|------|
| Tauri invoke wrappers | `src/lib/tauri.ts` | All backend calls go through typed wrappers |
| Entry + tags query | `library/entries.rs` | LEFT JOIN + GROUP_CONCAT for tag aggregation |
| Background downloads | `commands/download.rs` | Torrent manager with librqbit |
| File probing | `commands/media.rs` + `library/items.rs` | ffprobe JSON → structured MediaInfo |
| Export/Import | `commands/export.rs` | entries + entry_tags + relations → JSON (v3.0.0 format) |

## External Service Quirks

- **YIFY**: Official API migrated to `movies-api.accel.li/api/v2/` (old `yts.torrentbay.st` returns HTML instead of JSON)
- **OpenSubtitles**: REST API at `api.opensubtitles.com/api/v1`. Requires Api-Key header. Search is free; download needs JWT login (optional, 5/day without auth). Old XML-RPC (`api.opensubtitles.org`) is deprecated.
- **TMDB**: free API key from themoviedb.org

## Runtime Dependencies

| Tool | Used by | Default config key |
|------|---------|-------------------|
| `alass` / `alass-cli` | Subtitle alignment | `tools.alass` |
| `ffmpeg` + `ffprobe` | Subtitle extraction, media probe, library scan | `tools.ffmpeg` |
| `mpv` (optional) | Media playback | `tools.player` |

## Config

Path: `~/.config/blowup/config.toml`

```toml
[tools]
alass  = "alass"
ffmpeg = "ffmpeg"
player = "mpv"

[download]
max_concurrent = 3
enable_dht = true
persist_session = true

[tmdb]
api_key = ""

[opensubtitles]
api_key = ""

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
max_entries = 500

[sync]
endpoint = ""
bucket = ""
access_key = ""
secret_key = ""
```

## Database

SQLite at `{APP_DATA_DIR}/blowup.db`. Tables: `entries`, `entry_tags`, `relations`, `library_items`, `library_assets`, `downloads`.
