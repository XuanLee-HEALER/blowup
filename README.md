# blowup

> [中文版本点此链接](./README_zh.md)

![Version](https://img.shields.io/badge/Version-1.0.0-blue?style=for-the-badge) ![License](https://img.shields.io/badge/License-MIT-darkgreen?style=for-the-badge) ![Crates.io](https://img.shields.io/crates/v/blowup?style=for-the-badge)

> **Blow-Up [Michelangelo Antonioni, 1966]**: A fashion photographer unknowingly captures a death on film after following two lovers in a park.
>
> The best movie I've seen so far.

---

**blowup** is a command-line tool that automates the technical side of watching foreign films — searching for torrents, downloading, fetching and aligning subtitles — so you can spend more time on the film itself.

## Features

| Command | Description |
|---------|-------------|
| `blowup search` | Search YIFY for movie torrents by title and year |
| `blowup download` | Download a torrent or magnet link via aria2c |
| `blowup info` | Look up movie details (cast, director, overview) via TMDB |
| `blowup sub fetch` | Download subtitles from OpenSubtitles |
| `blowup sub align` | Auto-sync subtitle timing to the video using alass |
| `blowup sub extract` | Extract an embedded subtitle stream from a video container |
| `blowup sub list` | List all subtitle streams in a video file |
| `blowup sub shift` | Shift all subtitle timestamps by N milliseconds |
| `blowup tracker update` | Update local tracker list from a remote source |
| `blowup config` | Read and write tool configuration |

## Installation

```bash
cargo install blowup
```

**Runtime dependencies** (install separately):

| Tool | Required for | Install |
|------|-------------|---------|
| `aria2c` | `download` | `brew install aria2` / `apt install aria2` |
| `alass` / `alass-cli` | `sub align` | `brew install alass` / [GitHub releases](https://github.com/kaegi/alass/releases) |
| `ffmpeg` + `ffprobe` | `sub extract`, `sub list` | `brew install ffmpeg` |

## Quick Start

```bash
# 1. Configure API keys
blowup config set tmdb.api_key YOUR_TMDB_KEY   # free key at themoviedb.org
blowup config set tools.aria2c /usr/local/bin/aria2c
blowup config set tools.alass /usr/local/bin/alass-cli

# 2. Find a movie
blowup info "Blow-Up" --year 1966

# 3. Search for a torrent
blowup search "Blow-Up" --year 1966

# 4. Download it
blowup download "https://yts.bz/torrent/download/HASH" --output-dir ~/Movies

# 5. Fetch Chinese subtitles
blowup sub fetch ~/Movies/Blow-Up.mp4 --lang zh

# 6. Auto-align subtitle timing
blowup sub align ~/Movies/Blow-Up.mp4 ~/Movies/Blow-Up.zh.srt
```

## Commands

### `search`

Search YIFY for torrents. Results are sorted by quality (2160p → 1080p → 720p) then seed count.

```
blowup search <QUERY> [--year YEAR]
```

```
blowup search "Parasite" --year 2019
1: Parasite (2019) [1080p] seeds=312
   torrent: https://yts.bz/torrent/download/...
```

### `download`

Download a torrent file URL or magnet link using aria2c.

```
blowup download <TARGET> [--output-dir DIR]
```

Configure the aria2c binary path via `blowup config set tools.aria2c <path>`.

### `info`

Query TMDB for movie metadata: overview, genres, director, top cast.

```
blowup info <QUERY> [--year YEAR]
```

Requires a free TMDB API key: set it with `blowup config set tmdb.api_key <key>`.

### `sub fetch`

Search OpenSubtitles and download the best matching subtitle file next to the video.

```
blowup sub fetch <VIDEO> [--lang LANG]
```

Default language is `zh` (Chinese). The subtitle is saved as `<video-stem>.<lang>.srt`.

Supported language codes: `zh` (Chinese), `en` (English), `ja` (Japanese), `ko` (Korean), `fr` (French), `de` (German), `es` (Spanish).

### `sub align`

Use [alass](https://github.com/kaegi/alass) to automatically synchronize subtitle timing to the video. The original subtitle is preserved as `<name>.bak.srt`.

```
blowup sub align <VIDEO> <SRT>
```

Configure the alass binary path via `blowup config set tools.alass <path>`.

### `sub extract`

Extract an embedded subtitle stream from a video container to an SRT file (requires ffmpeg).

```
blowup sub extract <VIDEO> [--stream INDEX]
```

### `sub list`

List all subtitle streams in a video file (requires ffprobe).

```
blowup sub list <VIDEO>
```

### `sub shift`

Shift all timestamps in an SRT file by a fixed offset in milliseconds. Positive values delay, negative values advance.

```
blowup sub shift <SRT> <OFFSET_MS>
```

### `tracker update`

Fetch the latest BitTorrent tracker list from a remote source and save it locally.

```
blowup tracker update [--source URL]
```

### `config`

Read and write tool configuration stored at `~/.config/blowup/config.toml`.

```
blowup config set <KEY> <VALUE>
blowup config get <KEY>
blowup config list
```

**Available keys:**

| Key | Type | Description |
|-----|------|-------------|
| `tools.aria2c` | path | Path to the aria2c binary |
| `tools.alass` | path | Path to the alass or alass-cli binary |
| `tmdb.api_key` | string | TMDB API key (get one free at [themoviedb.org](https://www.themoviedb.org/settings/api)) |
| `opensubtitles.api_key` | string | OpenSubtitles API key (optional) |
| `subtitle.default_lang` | string | Default subtitle language code (e.g. `zh`) |
| `search.rate_limit_secs` | integer | Seconds to wait between search requests |

## License

MIT — see [LICENSE](./LICENSE.txt).
