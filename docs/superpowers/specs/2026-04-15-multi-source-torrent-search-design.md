# Multi-Source Torrent Search Design

**Status**: Draft
**Date**: 2026-04-15
**Scope**: Refactor `crates/core/src/torrent/search.rs` from single-source (YTS) into a pluggable multi-source pipeline with scoring.

---

## 1. Motivation

Current state: blowup only searches YTS via `yts.torrentbay.st` → `movies-api.accel.li`. Three concrete problems:

1. **Low seeder count.** YTS specializes in re-encoded small-bitrate releases; swarms are thin even for popular Western films. Adding more trackers to the torrent client doesn't fix this — the *source* of torrents is the bottleneck.
2. **No quality signal beyond `quality_rank × seeds`.** The current sort ignores source type (Bluray vs WEBRip vs CAM), release group reputation, size anomalies, codec, HDR.
3. **Poor Chinese cinema coverage.** YTS has minimal 华语片 coverage. A user looking for a Chinese film typically gets zero results.

Goals:
- Search multiple public sources concurrently and merge results.
- Score each result on multiple dimensions so the top entries are actually the best, not just "1080p with some seeds".
- Fail gracefully when a source is down — other sources still return results.
- Rate-limit per-source to avoid getting banned.

Non-goals (explicitly out of scope for this spec):
- Jackett / Prowlarr meta-search integration (future spec).
- Private-tracker support (requires auth, session management — future spec).
- Incremental / streaming result delivery (all results delivered at once when ready).
- User-configurable provider list or scoring weights (YAGNI; constants in code).

---

## 2. Architecture

### 2.1 Module layout

Replace the single file `crates/core/src/torrent/search.rs` with a module directory:

```
crates/core/src/torrent/search/
├── mod.rs              # Public API: search_movie() orchestrator
├── types.rs            # SearchQuery / SearchContext / RawTorrent / ScoredTorrent / enums
├── provider.rs         # SearchProvider trait + CallPacer + with_retry helper
├── parser.rs           # parse_release_title() — regex extraction
├── scorer.rs           # score() + ScoreBreakdown
├── dedup.rs            # merge() — group by info_hash
└── providers/
    ├── mod.rs          # re-exports + build_default_providers()
    ├── yts.rs
    ├── nyaa.rs
    └── onethreeseven.rs
```

### 2.2 The `SearchProvider` trait

```rust
#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn name(&self) -> &'static str;          // "yts" | "1337x" | "nyaa"
    fn min_interval(&self) -> Duration;       // per-provider rate limit
    fn max_retries(&self) -> u32 { 2 }
    async fn search(&self, ctx: &SearchContext<'_>) -> Result<Vec<RawTorrent>, ProviderError>;
}
```

Providers are **stateless**. No fields beyond static config (e.g., YTS's tmdb_api_key, which is a clone of the string from `SearchContext` — could also be kept in context). Construction cost is zero.

Each `search()` implementation internally creates a `CallPacer` (§2.4) for its own sub-requests and uses the `with_retry` helper (§2.5) to handle transient failures.

### 2.3 Types

```rust
pub struct SearchQuery {
    pub title: String,
    pub year: Option<u32>,
    pub tmdb_id: Option<u64>,
    pub tmdb_api_key: String,
}

pub struct SearchContext<'a> {
    pub http: &'a reqwest::Client,
    pub title: &'a str,
    pub year: Option<u32>,
    pub imdb_id: Option<&'a str>,       // resolved once in orchestrator
    pub tmdb_api_key: &'a str,
    pub trackers: &'a [String],          // from TrackerManager::hot_trackers()
}

pub struct RawTorrent {
    pub source: &'static str,
    pub raw_title: String,
    pub info_hash: Option<String>,       // lowercase hex
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub size_bytes: Option<u64>,
    pub seeders: u32,
    pub leechers: u32,
}

pub enum Resolution { Unknown, Sd, P480, P720, P1080, P2160 }
pub enum SourceKind { Unknown, Cam, Ts, Hdtv, WebRip, WebDl, Bluray, Remux }
pub enum Codec { Unknown, X264, X265, Av1 }

pub struct ParsedTitle {
    pub resolution: Resolution,
    pub source_kind: SourceKind,
    pub codec: Codec,
    pub release_group: Option<String>,
    pub hdr: bool,
}

pub struct ScoredTorrent {
    // from RawTorrent
    pub source: &'static str,
    pub raw_title: String,
    pub info_hash: Option<String>,
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub size_bytes: Option<u64>,
    pub seeders: u32,
    pub leechers: u32,
    // from ParsedTitle
    pub resolution: Resolution,
    pub source_kind: SourceKind,
    pub codec: Codec,
    pub release_group: Option<String>,
    pub hdr: bool,
    // computed
    pub score: i32,
    pub breakdown: ScoreBreakdown,
}

pub struct ScoreBreakdown {
    pub seeders: i32,
    pub resolution: i32,
    pub source: i32,
    pub codec: i32,
    pub size: i32,
    pub group: i32,
    pub hdr: i32,
}

impl ScoreBreakdown {
    pub fn total(&self) -> i32 {
        self.seeders + self.resolution + self.source + self.codec
            + self.size + self.group + self.hdr
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("network timeout")]
    Timeout,
    #[error("connect failed")]
    Connect,
    #[error("http 5xx: {0}")]
    Http5xx(u16),
    #[error("http 429 rate limited")]
    Http429,
    #[error("http 4xx: {0}")]
    Http4xx(u16),
    #[error("parse error: {0}")]
    Parse(String),
}

impl ProviderError {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout | Self::Connect | Self::Http5xx(_) | Self::Http429)
    }
}
```

### 2.4 `CallPacer` — per-task rate limiting

**Lifetime**: one instance per `search()` call (not shared across searches). Because blowup currently has no concurrent search and user click spacing naturally exceeds any reasonable `min_interval`, we only need pacing *within* a single search — specifically between retries and between sub-requests within one provider (e.g., 1337x search page → detail page).

```rust
pub struct CallPacer {
    min_interval: Duration,
    last: Option<Instant>,
}

impl CallPacer {
    pub fn new(min_interval: Duration) -> Self {
        Self { min_interval, last: None }
    }

    pub async fn wait(&mut self) {
        if let Some(prev) = self.last
            && prev.elapsed() < self.min_interval
        {
            tokio::time::sleep(self.min_interval - prev.elapsed()).await;
        }
        self.last = Some(Instant::now());
    }
}
```

Accessed via `&mut self` within a single async task → no `Mutex` needed.

### 2.5 `with_retry` helper

```rust
pub async fn with_retry<F, Fut, T>(
    pacer: &mut CallPacer,
    max_retries: u32,
    mut op: F,
) -> Result<T, ProviderError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ProviderError>>,
{
    let mut attempt: u32 = 0;
    loop {
        pacer.wait().await;
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) if !e.is_retryable() => return Err(e),
            Err(e) if attempt >= max_retries => return Err(e),
            Err(e) => {
                tracing::warn!(attempt, error = %e, "provider call failed, retrying");
                // exponential backoff, capped at 30s
                let backoff = Duration::from_secs(2u64.pow(attempt).min(30));
                tokio::time::sleep(backoff).await;
                attempt += 1;
            }
        }
    }
}
```

Error classification:
- `Timeout`, `Connect`, `Http5xx`, `Http429` → retryable
- `Http4xx` (other than 429) → not retryable (bad request; code bug)
- `Parse` → not retryable (code bug)

### 2.6 Orchestrator — `search_movie()`

```rust
pub async fn search_movie(
    http: &reqwest::Client,
    trackers: &Arc<TrackerManager>,
    query: SearchQuery,
) -> Vec<ScoredTorrent> {
    // 1. Resolve IMDB ID once (optional; YTS uses it)
    let imdb_id = if let Some(tmdb_id) = query.tmdb_id {
        fetch_imdb_id(http, &query.tmdb_api_key, tmdb_id).await
    } else {
        None
    };

    // 2. Snapshot tracker list once for Nyaa
    let tracker_list = trackers.hot_trackers().await;

    let ctx = SearchContext {
        http,
        title: &query.title,
        year: query.year,
        imdb_id: imdb_id.as_deref(),
        tmdb_api_key: &query.tmdb_api_key,
        trackers: &tracker_list,
    };

    // 3. Build provider list and run concurrently via join_all
    let providers = providers::build_default_providers();
    let futures = providers.iter().map(|p| async move {
        let t = Instant::now();
        let res = p.search(&ctx).await;
        tracing::debug!(
            provider = p.name(),
            elapsed_ms = t.elapsed().as_millis(),
            ok = res.is_ok(),
            "provider finished"
        );
        (p.name(), res)
    });
    let results = futures::future::join_all(futures).await;

    // 4. Collect successes, log failures (silent to caller)
    let mut all_raw = Vec::new();
    for (name, res) in results {
        match res {
            Ok(items) => {
                tracing::debug!(provider = name, count = items.len(), "provider returned");
                all_raw.extend(items);
            }
            Err(e) => {
                tracing::warn!(provider = name, error = %e, "provider failed");
            }
        }
    }

    // 5. Drop entries with no downloadable entrypoint (no magnet AND no torrent_url)
    let usable: Vec<RawTorrent> = all_raw
        .into_iter()
        .filter(|r| r.magnet.is_some() || r.torrent_url.is_some())
        .collect();

    // 6. Dedup by info_hash
    let deduped = dedup::merge(usable);

    // 7. Parse + score (see assemble_scored for the field-by-field copy)
    let mut scored: Vec<ScoredTorrent> = deduped
        .into_iter()
        .map(assemble_scored)
        .collect();

    // 8. Sort by total score desc
    scored.sort_by(|a, b| b.score.cmp(&a.score));

    scored
}

fn assemble_scored(raw: RawTorrent) -> ScoredTorrent {
    let parsed = parser::parse_release_title(&raw.raw_title);
    let breakdown = scorer::score(&raw, &parsed);
    let score = breakdown.total();
    ScoredTorrent {
        source: raw.source,
        raw_title: raw.raw_title,
        info_hash: raw.info_hash,
        magnet: raw.magnet,
        torrent_url: raw.torrent_url,
        size_bytes: raw.size_bytes,
        seeders: raw.seeders,
        leechers: raw.leechers,
        resolution: parsed.resolution,
        source_kind: parsed.source_kind,
        codec: parsed.codec,
        release_group: parsed.release_group,
        hdr: parsed.hdr,
        score,
        breakdown,
    }
}
```

Failure semantics:
- Individual provider failures are silently logged via `tracing::warn!` and never bubble up.
- All providers failing → `vec![]`; frontend renders "未找到资源".
- No `Result` wrapper: the orchestrator is total. Only truly unrecoverable internal errors (which don't exist in this design) would justify `Result`; there are none, so the signature is plain `Vec<ScoredTorrent>`.

---

## 3. Providers

### 3.1 YtsProvider

- **Endpoint**: `https://movies-api.accel.li/api/v2/list_movies.json?query_term=...&sort_by=seeds&order_by=desc&year=...`
- **min_interval**: 3s
- **Strategy**: existing 3-step fallback (IMDB ID → original title → sanitized title), lifted from current `search.rs`. IMDB ID is now pre-resolved in orchestrator.
- **Fields preserved**: title, year, each `torrents[]` entry → one `RawTorrent` with quality/size/seeds/magnet_url. `size_bytes` is now captured (previously dropped).
- **info_hash**: extracted from `magnet_url` via regex `urn:btih:([a-f0-9]+)`, lowercased.
- **When magnet is None**: `torrent_url` is still set; `info_hash` stays None.

### 3.2 NyaaProvider

- **Endpoint**: `https://nyaa.si/?page=rss&q=<urlencoded query>&c=0_0&f=0&s=seeders&o=desc`
- **Format**: RSS/XML. Each `<item>` has:
  - `<title>` — full release title
  - `<link>` — .torrent file URL
  - `<nyaa:infoHash>` — hex hash, no magnet
  - `<nyaa:seeders>`, `<nyaa:leechers>`, `<nyaa:size>` (e.g., "4.2 GiB")
- **Dependency**: `quick-xml = { version = "0.36", features = ["serde"] }`
- **min_interval**: 2s
- **Magnet construction**: Nyaa doesn't provide magnets. Build one manually from `info_hash` + `raw_title` + the `trackers` slice from `SearchContext` (the `TrackerManager::hot_trackers()` snapshot):

  ```rust
  fn make_magnet(info_hash: &str, title: &str, trackers: &[String]) -> String {
      let tr_params: String = trackers
          .iter()
          .map(|t| format!("&tr={}", urlencoding::encode(t)))
          .collect();
      format!(
          "magnet:?xt=urn:btih:{}&dn={}{}",
          info_hash,
          urlencoding::encode(title),
          tr_params,
      )
  }
  ```

- **Size parsing**: helper `parse_size_human("4.2 GiB")` → bytes. Handles MiB/GiB/TiB, no crate needed.
- **Query**: use raw `ctx.title`. Do **not** append year (Nyaa has no year filter; appending string "1966" often filters out valid results).
- **IMDB**: Nyaa doesn't index IMDB. `ctx.imdb_id` ignored.

### 3.3 OnethreesevenProvider (1337x)

Most complex provider. 1337x is HTML-scraped and requires a second request for each detail page to get the magnet.

- **Dependency**: `scraper = "0.20"`
- **User-Agent**: real browser UA string, e.g., `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36`
- **min_interval**: 5s (most conservative)
- **Two-stage search**:

  **Stage 1** — search listing page:
  ```
  GET https://1337x.to/search/<urlencoded query>/1/
  ```
  Parse the results table. Each row yields: `raw_title`, detail page URL, `seeders`, `leechers`, size (human-readable → bytes).

  **Stage 2** — detail pages:
  - Take top **5** entries by `seeders` from Stage 1.
  - Fetch all 5 concurrently via `futures::stream::iter(...).buffer_unordered(5)`. `CallPacer` does NOT gate within Stage 2 — 5 parallel requests to one host is acceptable and keeps total latency under ~5s.
  - Each detail page contains a `<a href="magnet:?xt=urn:btih:...">` anchor. Extract the magnet; extract `info_hash` from it.
  - Between Stage 1 and Stage 2, call `pacer.wait()` once to enforce `min_interval`.

- **Selectors**: define as module-level `&'static str` constants at the top of the file. On parse miss (selector returned 0 elements), return `ProviderError::Parse(...)` with a clear message so the user can file an issue.
- **Cloudflare**: no bypass attempts. If 403/503 after retries, return error — orchestrator silently drops this provider's results.
- **When the detail page is missing a magnet** (shouldn't happen, but possible): that entry keeps `torrent_url` = detail URL (download will work via the manager pulling the .torrent file), `info_hash` = None. Orchestrator's usability filter keeps it.

### 3.4 DEBUG tracing (all providers)

Every provider emits the following trace fields per call (at `debug` level):

| Field | Meaning |
|---|---|
| `provider` | `"yts" \| "1337x" \| "nyaa"` |
| `request_url` | actual URL being hit |
| `request_method` | `"GET" \| "POST"` |
| `response_ms` | elapsed from request send to body received |
| `response_status` | HTTP status code |
| `raw_count` | number of entries pre-parse |
| `parsed_ok` / `parsed_failed` | parse success / failure counters |
| `filter_reasons` | e.g., `dropped_no_magnet = 3` (structured fields) |

These logs are the primary debugging surface when adjusting a provider during implementation.

---

## 4. Parser (`parser.rs`)

Regex-based extraction from `raw_title`. All matching is case-insensitive (`(?i)` prefix or `to_lowercase()` first). Regexes are compiled once via `std::sync::LazyLock<Regex>`.

| Field | Regex (priority order) |
|---|---|
| resolution | `\b(2160p\|4k\|uhd)\b` → P2160; `\b1080p\b` → P1080; `\b720p\b` → P720; `\b480p\b` → P480 |
| source_kind | `\bremux\b` → Remux; `\b(bluray\|blu-?ray\|bdrip\|brrip)\b` → Bluray; `\bweb-?dl\b` → WebDl; `\bwebrip\b` → WebRip; `\bhdtv\b` → Hdtv; `\b(ts\|telesync)\b` → Ts; `\b(cam\|camrip\|hdcam)\b` → Cam |
| codec | `\b(x265\|h\.?265\|hevc)\b` → X265; `\b(x264\|h\.?264\|avc)\b` → X264; `\bav1\b` → Av1 |
| hdr | `\b(hdr\|hdr10\|dv\|dolby.?vision)\b` → true |
| release_group | `-([A-Z0-9]{2,})$` applied to the ORIGINAL (non-lowercased) title, taking only trailing `-GROUP` segments |

Unknown fields stay `Unknown` / `None` — scorer treats unknown as neutral (0 points).

---

## 5. Scorer (`scorer.rs`)

Additive integer model. `score = sum of breakdown fields`. All weights are hardcoded constants.

```rust
pub fn score(raw: &RawTorrent, p: &ParsedTitle) -> ScoreBreakdown {
    ScoreBreakdown {
        seeders: seeders_score(raw.seeders),
        resolution: resolution_score(p.resolution),
        source: source_score(p.source_kind),
        codec: codec_score(p.codec),
        size: size_score(raw.size_bytes, p.resolution, p.codec),
        group: group_score(p.release_group.as_deref()),
        hdr: if p.hdr { 30 } else { 0 },
    }
}
```

### 5.1 Per-dimension weights

**Seeders** (most reliable signal):
- `seeders < 3` → `-1000` (hard floor)
- `seeders ∈ [3, 100]` → `(seeders * 5) as i32`
- `seeders > 100` → `500` (cap)

**Resolution**:
| P2160 | P1080 | P720 | P480 | Sd | Unknown |
|---|---|---|---|---|---|
| 300 | 200 | 100 | 30 | 10 | 0 |

**Source**:
| Remux | Bluray | WebDl | WebRip | Hdtv | Ts/Cam | Unknown |
|---|---|---|---|---|---|---|
| 300 | 250 | 200 | 120 | 80 | -300 | 0 |

**Codec**:
| X265 | Av1 | X264 | Unknown |
|---|---|---|---|
| 20 | 20 | 10 | 0 |

**Size** (compare `raw.size_bytes` against expected for `(resolution, codec)`):

Expected midpoints:
| Resolution | x264 | x265 / AV1 |
|---|---|---|
| 720p  | 4 GB  | 1.5 GB |
| 1080p | 10 GB | 4 GB   |
| 2160p | 40 GB | 25 GB  |

- Unknown codec → treat as x264.
- Unknown resolution → return 0 (no signal).
- Unknown size → return 0.
- `ratio = actual / expected`:
  - `ratio < 0.3` → `-150` (heavy compression, likely garbage)
  - `0.3 ≤ ratio < 0.5` → `-50`
  - `0.5 ≤ ratio ≤ 1.5` → `0` (healthy range)
  - `1.5 < ratio ≤ 2.0` → `0`
  - `ratio > 2.0` → `-50` (bloated, no quality benefit)

**Release group**:
- Whitelist (+50): `SPARKS`, `GECKOS`, `AMIABLE`, `FraMeSToR`, `RARBG`, `NTb`, `CMRG`, `KOGi`, `PSA`
- Blacklist (-100): `Ganool`, `ETRG`
- Everything else (including `YTS`, `YIFY`, unknown): `0`

Group names compared case-insensitively.

**HDR**:
- `hdr = true` → `30`
- otherwise → `0`

### 5.2 Why this balance

- Seeders max 500 ≈ Bluray source (250) + 1080p (200) + small extras. A high-seed WEB-DL ties a low-seed Bluray. User can override by reading the breakdown.
- No hard filters beyond `seeders < 3`. The user sees everything, ranked.

---

## 6. Dedup (`dedup.rs`)

```rust
pub fn merge(raws: Vec<RawTorrent>) -> Vec<RawTorrent> {
    let mut by_hash: HashMap<String, RawTorrent> = HashMap::new();
    let mut without_hash: Vec<RawTorrent> = Vec::new();

    for r in raws {
        match r.info_hash.clone() {
            Some(h) => {
                by_hash
                    .entry(h)
                    .and_modify(|existing| merge_into(existing, &r))
                    .or_insert(r);
            }
            None => without_hash.push(r),
        }
    }

    let mut out: Vec<RawTorrent> = by_hash.into_values().collect();
    out.extend(without_hash);
    out
}

fn merge_into(existing: &mut RawTorrent, new: &RawTorrent) {
    existing.seeders = existing.seeders.max(new.seeders);
    existing.leechers = existing.leechers.max(new.leechers);
    if existing.magnet.is_none() && new.magnet.is_some() {
        existing.magnet = new.magnet.clone();
    }
    if existing.torrent_url.is_none() && new.torrent_url.is_some() {
        existing.torrent_url = new.torrent_url.clone();
    }
    // source tag kept as first-seen; multi-source origin only visible in trace logs
}
```

- **Key is `info_hash`**, nothing else. No title fuzzy matching — too error-prone ("X 1080p" vs "X 720p" should NOT merge).
- **Entries with no hash** are kept as independent results.
- **Entries with no downloadable entrypoint** (no magnet AND no torrent_url) are dropped in the orchestrator BEFORE calling dedup.

---

## 7. Frontend changes

### 7.1 Type definitions (`src/lib/tauri.ts`)

**Remove**: `MovieResult` (replaced by `ScoredTorrent`).

**Add**:

```ts
export type Resolution = "unknown" | "sd" | "p480" | "p720" | "p1080" | "p2160";
export type SourceKind =
  | "unknown" | "cam" | "ts" | "hdtv" | "webrip" | "webdl" | "bluray" | "remux";
export type Codec = "unknown" | "x264" | "x265" | "av1";

export interface ScoreBreakdown {
  seeders: number; resolution: number; source: number;
  codec: number;   size: number;       group: number; hdr: number;
}

export interface ScoredTorrent {
  source: string;
  raw_title: string;
  info_hash: string | null;
  magnet: string | null;
  torrent_url: string | null;
  size_bytes: number | null;
  seeders: number;
  leechers: number;
  resolution: Resolution;
  source_kind: SourceKind;
  codec: Codec;
  release_group: string | null;
  hdr: boolean;
  score: number;
  breakdown: ScoreBreakdown;
}
```

**Rename** the invoke wrapper `yts.search(...)` → `search.movie(...)`, target command `search_movie_cmd`.

**Remove** the dead config field — delete the `search` section from `AppConfig` entirely:
```ts
// Delete: search: { rate_limit_secs: number };
```
No TS-side replacement; the whole `search` key is gone from `AppConfig`.

### 7.2 `FilmDetailPanel.tsx` — `TorrentSearchModal` UI

Redesign each result row to show:

```
┌──────────────────────────────────────────────────────────────────┐
│ ⭐ 820   1080p · Bluray · x265 · HDR       12.3 GB   ▸ 214 seeds │
│         [nyaa] Blow-Up.1966.1080p.BluRay.x265-FraMeSToR          │
│                                                    [详情] [下载] │
└──────────────────────────────────────────────────────────────────┘
```

- **Score badge**: `⭐ {score}` as a Mantine Badge.
- **Quality line**: human-readable translation of `resolution` / `source_kind` / `codec` / `hdr` joined by `·`. Translations via small local helpers:
  ```ts
  const resolutionLabel = (r: Resolution) => ({ p2160: "4K", p1080: "1080p", ... })[r] ?? "?";
  ```
- **Size**: existing `formatSize()` helper from `src/lib/format.ts`.
- **Second line**: `[{source}]` tag + truncated `raw_title`.
- **[详情] button**: toggles an inline accordion showing the 7 breakdown fields + total:
  ```
  seeders     +500  (214 peers)
  resolution  +200  (1080p)
  source      +250  (Bluray)
  codec        +20  (x265)
  size          +0  (12.3 GB, in range)
  group        +50  (FraMeSToR)
  hdr          +30  (HDR)
  total       +1050
  ```
- **[下载] button**: unchanged behavior — passes `r.magnet ?? r.torrent_url` to the existing `download.getTorrentFiles` → `download.startDownload` flow.

### 7.3 Loading state

Current loading text `搜索中...` becomes `搜索中... (YTS · Nyaa · 1337x)` — static, no per-provider progress. Since total latency can reach 15-20s with retries, the static hint helps the user understand it's not hung.

### 7.4 Empty / all-failed

`results.length === 0` → existing `未找到资源` text. No distinction between "all providers returned empty" and "all providers failed" — both are silent per user requirements.

### 7.5 Settings — remove `rate_limit_secs` input

`src/pages/Settings.tsx` currently has a number input bound to `c.search.rate_limit_secs`. Delete that UI block and the corresponding handler. This was a dead field with no consumer in the code.

---

## 8. Tauri / Server API

### 8.1 Tauri command

**Rename**: `search_yify_cmd` → `search_movie_cmd`.

```rust
#[tauri::command]
pub async fn search_movie_cmd(
    query: String,
    year: Option<u32>,
    tmdb_id: Option<u64>,
    ctx: State<'_, Arc<AppContext>>,
) -> Result<Vec<ScoredTorrent>, String> {
    let cfg = blowup_core::config::load_config();
    let q = SearchQuery {
        title: query,
        year,
        tmdb_id,
        tmdb_api_key: cfg.tmdb.api_key,
    };
    Ok(search_movie(&ctx.http, &ctx.tracker, q).await)
}
```

Note: `AppContext` already owns both `http: reqwest::Client` and `tracker: Arc<TrackerManager>` as public fields (verified in `crates/core/src/context.rs`). The Tauri command reuses them — replacing the per-call `reqwest::Client::new()` waste and avoiding a second `State<_>` parameter.

### 8.2 Server route

- Path: `/api/v1/search/yify` → `/api/v1/search/movie`
- Request body unchanged structurally: `{ query, year, tmdb_id }`
- Response body: `Vec<ScoredTorrent>`

### 8.3 Old surface

Delete:
- `search_yify_cmd` (Tauri)
- `search_yify` (server route)
- `yts.search` wrapper in `src/lib/tauri.ts`
- `MovieResult` interface in `src/lib/tauri.ts`
- `rate_limit_secs` field in config + Settings UI

---

## 9. Config changes

**Remove**:
- `crates/core/src/config/mod.rs`: the entire `SearchConfig` struct (it has only the one dead field), the `pub search: SearchConfig` line in `Config`, `default_rate_limit()`, and related tests that assert `cfg.search.rate_limit_secs`.
- `src/pages/Settings.tsx`: the number input for `rate_limit_secs` plus its surrounding section heading if that was the only control in it.
- `src/lib/tauri.ts`: the entire `search` key from the `AppConfig` interface.

**Compatibility**: existing `config.toml` files with `[search] rate_limit_secs = 5` still load cleanly because serde ignores unknown fields by default. Next `save_config` call strips the field.

**Add**: nothing. Provider list is a compile-time constant (`build_default_providers()`). Rate limits and scoring weights are hardcoded.

---

## 10. New Rust dependencies

Added to `crates/core/Cargo.toml`:

| Crate | Version | Used by | Purpose |
|---|---|---|---|
| `async-trait` | 0.1 | provider.rs | `#[async_trait]` on `SearchProvider` |
| `quick-xml` | 0.36 (with `serde`) | nyaa.rs | RSS/XML parsing |
| `scraper` | 0.20 | onethreeseven.rs | HTML scraping |
| `futures` | 0.3 | mod.rs, onethreeseven.rs | `join_all`, `buffer_unordered` |

(`urlencoding` is already a dependency; reused for query escaping.)

---

## 11. Tests

### 11.1 Unit tests (run in `just test`)

- **`parser.rs`**: ~20 golden release-title fixtures covering: YTS format, scene format, Nyaa Chinese releases, titles with dots vs spaces, titles with HDR/DV, titles without group, titles with release group.
- **`scorer.rs`**: boundary tests per dimension — seeders at 2/3/100/200, each resolution/source/codec variant, size ratios at 0.2/0.4/1.0/1.8/2.5, known whitelist/blacklist groups.
- **`dedup.rs`**: empty, single, multi-source same hash, multi-source different hashes, no-hash entries preserved, `merge_into` field merging rules.
- **Providers (parse only)**: each provider has a fixture-based parse test:
  - `yts.rs`: embed a sample `YtsResponse` JSON → parse → assert `Vec<RawTorrent>` shape.
  - `nyaa.rs`: fixture file `fixtures/nyaa_search.xml` → parse → assert.
  - `onethreeseven.rs`: fixture files `fixtures/1337x_search.html` + `fixtures/1337x_detail.html` → assert.
- **Orchestrator**: mock provider trait impls (stub `Vec<RawTorrent>`), verify: join_all runs both, one failing provider doesn't stop others, dedup applies, sort is correct.

### 11.2 Live smoke tests (business-independent, `#[ignore]` by default)

Each provider gets ONE `#[tokio::test] #[ignore]` test that hits the real site with a fixed generic query (`"The Matrix"`, 1999). Assertions are **structural only**:
- `results.len() >= 1`
- First result has `info_hash.is_some() || torrent_url.is_some()`
- `seeders` is a valid `u32` (always true; the point is the test didn't crash)

NO assertions on specific scores, release groups, ranking, or sort order — those are business logic that changes with live data.

Run manually: `cargo test -p blowup-core --ignored search_providers_live_`.

Purpose: detect when a site changes its HTML/API schema, so we know to re-adjust the parser. These tests are **not** run in CI — network dependency + site flakiness would cause false failures.

---

## 12. Implementation order

Suggested bottom-up sequence (each step independently compilable and testable):

1. **Types + enums** (`types.rs`) — no dependencies, no tests needed beyond compile.
2. **Parser** (`parser.rs`) + unit tests — pure function, golden fixtures.
3. **Scorer** (`scorer.rs`) + unit tests — pure function on `(RawTorrent, ParsedTitle)`.
4. **Dedup** (`dedup.rs`) + unit tests.
5. **Provider trait + CallPacer + with_retry** (`provider.rs`).
6. **YtsProvider** — port existing code into the new trait, keep existing YTS tests.
7. **NyaaProvider** + parse test + live smoke test.
8. **OnethreesevenProvider** + parse test + live smoke test.
9. **Orchestrator** (`mod.rs`) + mock-provider test.
10. **Wire into Tauri + server** — rename commands, update AppContext usage, update server route.
11. **Frontend** — update TS types, update `FilmDetailPanel.tsx`, remove dead `rate_limit_secs` UI.
12. **Config cleanup** — delete `rate_limit_secs` field + tests.

Each step passes `just check`.

---

## 13. Risks and mitigations

| Risk | Mitigation |
|---|---|
| 1337x adds full Cloudflare challenge → 100% failure | Log warning, orchestrator silently drops; YTS + Nyaa still return results. Future: Jackett integration. |
| 1337x selector changes breaking parser | Live smoke test detects schema change; parse error returns `ProviderError::Parse` with clear message. |
| Nyaa RSS schema change | Live smoke test detects; structured serde deserialization will fail loudly. |
| YTS mirror (`movies-api.accel.li`) goes down | Pre-existing risk, unchanged by this refactor; provider simply returns error. |
| Rate-limit tripped despite pacer | Unlikely within a single search task; if observed, tighten `min_interval` constants (one-line change). |
| Scoring weights feel wrong in practice | Weights are all constants in `scorer.rs`; tuning is a localized edit. User feedback loop after first release. |
| 15-20s worst-case latency frustrates user | Static "搜索中... (YTS · Nyaa · 1337x)" text sets expectation; most searches are faster (parallel). |

---

## 14. Out of scope — explicit non-goals

- Jackett / Prowlarr integration (future spec).
- Private-tracker support (MTeam / HDChina / CHDBits).
- Per-user scoring preference tuning.
- Provider enable/disable UI.
- Streaming / incremental result delivery.
- CAPTCHA / Cloudflare bypass.
- Caching search results across queries.
- Persistent app-level rate limiter.
