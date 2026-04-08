# CLAUDE.md

## Project Overview

**blowup** v2.0.2 — A Tauri v2 desktop app for the Chinese film-watching pipeline: TMDB discovery, torrent search & download, subtitle management, personal film knowledge base, and media playback.

Named after Michelangelo Antonioni's 1966 film *Blow-Up*.

GitHub: https://github.com/XuanLee-HEALER/blowup

## Architecture

Dual codebase: Rust backend (Tauri commands + SQLite) and React 19 frontend (TypeScript + Vite).

```
src-tauri/                    # Rust backend (Tauri v2)
├── src/
│   ├── lib.rs               # Tauri app builder, command registration (~50 commands)
│   ├── config.rs             # Config struct, TOML ser/de, load_config(), save_config()
│   ├── db/mod.rs             # SQLite pool init (sqlx + migrations)
│   ├── error.rs              # thiserror enums per domain
│   ├── ffmpeg.rs             # FfmpegTool wrapper (ffmpeg/ffprobe)
│   ├── common.rs             # exec_command, find_command_path
│   └── commands/
│       ├── config.rs         # get_config, set_config_key, set_music_playlist
│       ├── search.rs         # YTS torrent search (movies-api.accel.li)
│       ├── download.rs       # aria2c wrapper + background download management
│       ├── tmdb.rs           # TMDB search/discover/credits
│       ├── tracker.rs        # BitTorrent tracker list update (octocrab)
│       ├── subtitle.rs       # fetch/align/extract/list/shift subtitles
│       ├── media.rs          # probe_media_detail, open_in_player
│       └── library/          # Knowledge base CRUD
│           ├── mod.rs        # Shared types + wiki helpers
│           ├── films.rs      # Film CRUD + list_films_filtered (QueryBuilder)
│           ├── people.rs     # Person CRUD + relations
│           ├── genres.rs     # Genre CRUD + hierarchical tree
│           ├── reviews.rs    # Review CRUD
│           ├── items.rs      # Library items + scan + assets + stats
│           └── graph.rs      # D3 graph data (person-film links)
├── migrations/
│   ├── 001_initial.sql       # people, films, genres, junction tables, wiki, reviews, library_items/assets
│   └── 002_downloads.sql     # downloads table
└── Cargo.toml

src/                          # React 19 frontend (TypeScript)
├── App.tsx                   # Router + sidebar nav + MusicPlayer
├── lib/
│   ├── tauri.ts              # All Tauri invoke wrappers + TS types
│   └── format.ts             # Shared formatters (size, duration, bitrate)
├── pages/
│   ├── Search.tsx            # TMDB search/discover with filters
│   ├── People.tsx            # Person list + detail + wiki + relations
│   ├── Genres.tsx            # Genre tree + detail
│   ├── Graph.tsx             # D3 force simulation + orbital rotation
│   ├── Library.tsx           # Film grid + file linking + stats + scan
│   ├── Download.tsx          # Download queue + history + manual add
│   ├── Subtitle.tsx          # Subtitle tools (fetch/align/extract/shift)
│   ├── Media.tsx             # Media probe + player launch
│   └── Settings.tsx          # Config editor
└── components/
    ├── FilmDetailPanel.tsx   # TMDB film detail + add to KB + YTS search modal
    ├── ReviewSection.tsx     # Film reviews (personal + external)
    ├── WikiEditor.tsx        # Markdown editor with DOMPurify
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
- Centered on **film genres** and **filmmakers (people)**, not individual films
- Tables: `people`, `genres`, `person_films`, `film_genres`, `reviews`, `wiki_entries`
- The `films` table exists to link people/genres together, NOT as the primary film data store
- Wiki content is for people and genres, not for individual films
- Pages: People.tsx, Genres.tsx, Graph.tsx

### Film Library (电影库) — File System
- Storage: `{library.root_dir}/{director}/{tmdb_id}/` directories on disk
- Index: `library_index.json` — in-memory `IndexEntry` array, persisted to JSON
- Each `IndexEntry` contains: `tmdb_id`, `title`, `director`, `year`, `genres`, `path`, `files[]`, and **enriched TMDB data** (poster, overview, cast) cached in the index
- Enriched data is **lazy-loaded**: if fields are empty on first view, fetch from TMDB API and cache in the index
- Pages: Library.tsx (director tree + detail panel)
- **Film detail page data comes from the file index, NOT from SQLite**

### The only connection
If a film mentioned in the knowledge base (e.g. in a person's filmography) also exists in the film library, a hyperlink can navigate to the corresponding Library detail page. That's it.

## Key Patterns

| Pattern | Location | Note |
|---------|----------|------|
| Tauri invoke wrappers | `src/lib/tauri.ts` | All backend calls go through typed wrappers |
| Wiki upsert | `library/mod.rs` | `upsert_wiki()` / `get_wiki_content()` shared helpers |
| Film filtering | `library/films.rs` | `QueryBuilder` for dynamic SQL with bound params |
| Background downloads | `commands/download.rs` | `tokio::spawn` monitors aria2c, atomic `UPDATE WHERE status='downloading'` |
| File probing | `commands/media.rs` + `library/items.rs` | ffprobe JSON → structured MediaInfo/VideoProbe |

## External Service Quirks

- **YIFY**: Official API migrated to `movies-api.accel.li/api/v2/` (old `yts.torrentbay.st` returns HTML instead of JSON)
- **OpenSubtitles**: REST API at `api.opensubtitles.com/api/v1`. Requires Api-Key header. Search is free; download needs JWT login (optional, 5/day without auth). Old XML-RPC (`api.opensubtitles.org`) is deprecated.
- **TMDB**: free API key from themoviedb.org

## Runtime Dependencies

| Tool | Used by | Default config key |
|------|---------|-------------------|
| `aria2c` | Downloads | `tools.aria2c` |
| `alass` / `alass-cli` | Subtitle alignment | `tools.alass` |
| `ffmpeg` + `ffprobe` | Subtitle extraction, media probe, library scan | `tools.ffmpeg` |
| `mpv` (optional) | Media playback | `tools.player` |

## Config

Path: `~/.config/blowup/config.toml`

```toml
[tools]
aria2c = "aria2c"
alass  = "alass"
ffmpeg = "ffmpeg"
player = "mpv"

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
```

## Database

SQLite at `{APP_DATA_DIR}/blowup.db`. Tables: `people`, `films`, `genres`, `person_films`, `film_genres`, `person_genres`, `person_relations`, `wiki_entries`, `reviews`, `library_items`, `library_assets`, `downloads`.
