# CLAUDE.md

## Project Overview

**blowup** v1.0.0 — A CLI tool for automating the Chinese film-watching pipeline: torrent search → download → subtitle fetch → subtitle alignment.

Named after Michelangelo Antonioni's 1966 film *Blow-Up*.

Published on crates.io: `cargo install blowup`
GitHub: https://github.com/XuanLee-HEALER/blowup

## Architecture

```
src/
├── main.rs          # clap CLI wiring (all subcommands → module functions)
├── lib.rs           # pub mod declarations
├── config.rs        # Config struct, TOML deserialization, config_path()
├── config_cmd.rs    # set/get/list config commands (toml_edit, KNOWN_KEYS)
├── error.rs         # typed errors: SearchError, DownloadError, SubError,
│                    #   TmdbError, ConfigCmdError, ConfigError
├── search.rs        # YIFY torrent search via yts.torrentbay.st API
├── download.rs      # aria2c download wrapper
├── tmdb.rs          # TMDB movie info (2-step: search → details+credits)
├── tracker.rs       # tracker list update via octocrab
└── sub/
    ├── mod.rs       # extract/list subtitle streams (ffmpeg/ffprobe)
    ├── fetch.rs     # OpenSubtitles XML-RPC subtitle download
    ├── align.rs     # alass subtitle alignment
    └── shift.rs     # manual timestamp offset
```

Config file location: `~/.config/blowup/config.toml`

## Development Commands

```bash
# Build
cargo build

# Run tests
cargo test

# Run
cargo run -- <subcommand>

# Lint
cargo clippy

# Format
cargo fmt

# Publish
cargo publish
```

## Code Style & Conventions

- Errors: one `thiserror` enum per domain in `error.rs`; `anyhow` only in `main`
- No `unwrap()` in non-test code; propagate errors with `?`
- Async only where needed (reqwest, tokio); sync where possible (file I/O, alass)
- Tests live in `#[cfg(test)] mod tests` at bottom of each file
- All user-facing strings in Chinese (the tool is for Chinese users)
- Commit messages follow conventional commits (`feat:`, `fix:`, `docs:`, `chore:`)

## Key Files

| File | Purpose |
|------|---------|
| `src/search.rs` | YIFY search — uses `yts.torrentbay.st` (not `yts.mx`, which blocks proxy) |
| `src/sub/fetch.rs` | Subtitle fetch — XML-RPC anonymous login; strips `/sid-TOKEN/` from download URLs |
| `src/tmdb.rs` | TMDB — two-step: `GET /3/search/movie` then `GET /3/movie/{id}?append_to_response=credits` |
| `src/config_cmd.rs` | Config — `KNOWN_KEYS` table drives validation, type coercion (Str/U64), and list display |
| `src/config.rs` | Config struct with serde defaults; `config_path()` → `~/.config/blowup/config.toml` |

## Notes

### External service quirks
- **YIFY**: `yts.mx` blocks connections from VLESS proxy exit IPs; use `yts.torrentbay.st` instead
- **OpenSubtitles**: new REST API (`api.opensubtitles.com`) requires paid API key; use XML-RPC at `api.opensubtitles.org/xml-rpc` for anonymous access. Download URLs from XML-RPC responses contain `/sid-TOKEN/` — strip it before downloading, otherwise you get a VIP-only stub
- **TMDB**: requires free API key from themoviedb.org; configure with `blowup config set tmdb.api_key <key>`

### Runtime dependencies (not bundled)
- `aria2c` — for `download` command
- `alass` / `alass-cli` — for `sub align` command
- `ffmpeg` + `ffprobe` — for `sub extract` / `sub list` commands

### Config keys
```toml
[tools]
aria2c = "/opt/homebrew/bin/aria2c"
alass  = "/opt/homebrew/bin/alass-cli"

[tmdb]
api_key = "..."

[opensubtitles]
api_key = ""          # optional

[subtitle]
default_lang = "zh"

[search]
rate_limit_secs = 0   # u64
```
