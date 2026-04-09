import { useState, useEffect, useRef, useCallback } from "react";
import { TextInput } from "../components/ui/TextInput";
import { Chip } from "../components/ui/Chip";
import { tmdb, config, type MovieListItem, type TmdbGenre, type SearchFilters } from "../lib/tauri";
import { FilmDetailPanel } from "../components/FilmDetailPanel";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

const SORT_OPTIONS = [
  { value: "vote_average.desc", label: "按评分排序" },
  { value: "popularity.desc",   label: "按热度排序" },
  { value: "release_date.desc", label: "按年份排序" },
];

// ── Filter Popover ───────────────────────────────────────────────

function FilterPopover({ anchor, onClose, children }: {
  anchor: string; onClose: () => void; children: React.ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [onClose]);

  return (
    <div ref={ref} style={{
      position: "absolute", top: "100%", left: 0, marginTop: 4, zIndex: 50,
      background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)",
      borderRadius: 8, padding: "0.75rem", minWidth: 180,
      boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
    }}>
      <p style={{ margin: "0 0 0.5rem", fontSize: "0.68rem", color: "var(--color-label-quaternary)", textTransform: "uppercase", letterSpacing: "0.04em" }}>
        {anchor}
      </p>
      {children}
    </div>
  );
}

// ── Individual filter components ─────────────────────────────────

const YEAR_MIN = 1920;
const YEAR_MAX = new Date().getFullYear();
const YEARS = Array.from({ length: YEAR_MAX - YEAR_MIN + 1 }, (_, i) => YEAR_MIN + i);

const selectStyle: React.CSSProperties = {
  padding: "0.3rem 0.4rem", fontSize: "0.78rem",
  background: "var(--color-bg-control)", border: "1px solid var(--color-separator)",
  borderRadius: 4, color: "var(--color-label-primary)", fontFamily: "inherit", outline: "none",
};

function YearFilter({ yearFrom, yearTo, onChange }: {
  yearFrom?: number; yearTo?: number;
  onChange: (from?: number, to?: number) => void;
}) {
  const fromOptions = YEARS.filter((y) => !yearTo || y <= yearTo);
  const toOptions = YEARS.filter((y) => !yearFrom || y >= yearFrom);
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
      <select
        value={yearFrom ?? ""}
        onChange={(e) => onChange(e.target.value ? Number(e.target.value) : undefined, yearTo)}
        style={selectStyle}
      >
        <option value="">起始</option>
        {fromOptions.map((y) => <option key={y} value={y}>{y}</option>)}
      </select>
      <span style={{ color: "var(--color-label-quaternary)", fontSize: "0.75rem" }}>—</span>
      <select
        value={yearTo ?? ""}
        onChange={(e) => onChange(yearFrom, e.target.value ? Number(e.target.value) : undefined)}
        style={selectStyle}
      >
        <option value="">结束</option>
        {toOptions.map((y) => <option key={y} value={y}>{y}</option>)}
      </select>
    </div>
  );
}

function GenreFilter({ genres, selected, onChange }: {
  genres: TmdbGenre[]; selected: number[]; onChange: (ids: number[]) => void;
}) {
  const [ids, setIds] = useState<Set<number>>(new Set(selected));
  const toggle = (id: number) => {
    const next = new Set(ids);
    if (next.has(id)) next.delete(id); else next.add(id);
    setIds(next);
  };
  return (
    <div>
      <div style={{ maxHeight: 200, overflowY: "auto", display: "flex", flexDirection: "column", gap: "0.15rem" }}>
        {genres.map((g) => (
          <label key={g.id} style={{ display: "flex", alignItems: "center", gap: "0.4rem", padding: "0.2rem 0", cursor: "pointer", fontSize: "0.78rem", color: "var(--color-label-secondary)" }}>
            <input type="checkbox" checked={ids.has(g.id)} onChange={() => toggle(g.id)}
              style={{ accentColor: "var(--color-accent)" }} />
            {g.name}
          </label>
        ))}
      </div>
      <button onClick={() => onChange([...ids])}
        style={{ marginTop: "0.5rem", background: "var(--color-accent)", border: "none", borderRadius: 4, padding: "0.25rem 0.6rem", color: "#fff", fontSize: "0.72rem", cursor: "pointer", fontFamily: "inherit", fontWeight: 600, width: "100%" }}>
        确定
      </button>
    </div>
  );
}

const RATINGS = Array.from({ length: 20 }, (_, i) => (i + 1) * 0.5); // 0.5 ~ 10.0

function RatingFilter({ current, onChange }: { current?: number; onChange: (v?: number) => void }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
      <span style={{ fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>≥</span>
      <select
        value={current ?? ""}
        onChange={(e) => onChange(e.target.value ? Number(e.target.value) : undefined)}
        style={selectStyle}
      >
        <option value="">不限</option>
        {RATINGS.map((r) => <option key={r} value={r}>{r.toFixed(1)}</option>)}
      </select>
    </div>
  );
}

function SortFilter({ current, onChange }: { current: string; onChange: (v: string) => void }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "0.15rem" }}>
      {SORT_OPTIONS.map((o) => (
        <label key={o.value} style={{ display: "flex", alignItems: "center", gap: "0.4rem", padding: "0.25rem 0", cursor: "pointer", fontSize: "0.78rem", color: current === o.value ? "var(--color-accent)" : "var(--color-label-secondary)" }}>
          <input type="radio" name="sort" checked={current === o.value} onChange={() => onChange(o.value)}
            style={{ accentColor: "var(--color-accent)" }} />
          {o.label}
        </label>
      ))}
    </div>
  );
}

// ── Main page ────────────────────────────────────────────────────

export default function Search() {
  const [apiKey, setApiKey] = useState("");
  const [query, setQuery] = useState("");
  const [genres, setGenres] = useState<TmdbGenre[]>([]);
  const [results, setResults] = useState<MovieListItem[]>([]);
  const [selected, setSelected] = useState<MovieListItem | null>(null);
  const [loading, setLoading] = useState(false);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);

  // Filters
  const [yearFrom, setYearFrom] = useState<number | undefined>();
  const [yearTo, setYearTo]     = useState<number | undefined>();
  const [genreIds, setGenreIds] = useState<number[]>([]);
  const [minRating, setMinRating] = useState<number | undefined>();
  const [sortBy, setSortBy]     = useState("vote_average.desc");

  // Popover state
  const [openFilter, setOpenFilter] = useState<string | null>(null);

  const loadApiConfig = useCallback(() => {
    config.get().then((cfg) => {
      setApiKey(cfg.tmdb.api_key);
      if (cfg.tmdb.api_key) {
        tmdb.listGenres(cfg.tmdb.api_key).then(setGenres).catch(() => {});
      }
    });
  }, []);

  useEffect(loadApiConfig, [loadApiConfig]);
  useBackendEvent(BackendEvent.CONFIG_CHANGED, loadApiConfig);

  const buildFilters = useCallback(
    (p = 1): SearchFilters => ({
      year_from: yearFrom,
      year_to: yearTo,
      genre_ids: genreIds,
      min_rating: minRating,
      sort_by: sortBy,
      page: p,
    }),
    [yearFrom, yearTo, genreIds, minRating, sortBy]
  );

  const runSearch = useCallback(
    async (q: string, p: number, append: boolean) => {
      if (!apiKey) return;
      setLoading(true);
      try {
        const filters = buildFilters(p);
        const items = q.trim()
          ? await tmdb.searchMovies(apiKey, q.trim(), filters)
          : await tmdb.discoverMovies(apiKey, filters);
        setResults(append ? (prev) => [...prev, ...items] : items);
        setPage(p);
        setHasMore(items.length >= 20);

        // Async enrich credits for top 3 results
        const top3Ids = items.slice(0, 3).map((m) => m.id);
        if (top3Ids.length > 0) {
          tmdb.enrichCredits(apiKey, top3Ids).then((credits) => {
            const map = new Map(credits.map((c) => [c.id, c]));
            setResults((prev) =>
              prev.map((m) => {
                const c = map.get(m.id);
                return c ? { ...m, director: c.director ?? undefined, cast: c.cast } : m;
              })
            );
          }).catch(() => { /* credits enrichment is best-effort */ });
        }
      } catch { /* */ } finally {
        setLoading(false);
      }
    },
    [apiKey, buildFilters]
  );

  const doSearch = useCallback(() => runSearch(query, 1, false), [query, runSearch]);

  const loadMore = () => runSearch(query, page + 1, true);

  const selectedGenreNames = genres
    .filter((g) => genreIds.includes(g.id))
    .map((g) => g.name);

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      {/* Left: search + list */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
        {/* Header */}
        <div style={{ padding: "1.4rem 1.5rem 0" }}>
          <h1
            style={{
              fontSize: "1.6rem",
              fontWeight: 700,
              letterSpacing: "-0.035em",
              marginBottom: "1.1rem",
            }}
          >
            搜索
          </h1>

          <TextInput
            leadingIcon="⌕"
            placeholder="电影名称、导演… (回车搜索)"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.nativeEvent.isComposing) doSearch(); }}
            style={{ marginBottom: "0.7rem" }}
          />

          {/* Filter chips */}
          <div style={{ display: "flex", gap: "0.4rem", flexWrap: "wrap", marginBottom: "1rem", position: "relative" }}>
            {/* Year range */}
            <div style={{ position: "relative" }}>
              <Chip
                label={yearFrom || yearTo ? `${yearFrom ?? "?"} – ${yearTo ?? "?"}` : "年代"}
                active={!!(yearFrom || yearTo)}
                onRemove={yearFrom || yearTo ? () => { setYearFrom(undefined); setYearTo(undefined); } : undefined}
                onClick={() => setOpenFilter(openFilter === "year" ? null : "year")}
              />
              {openFilter === "year" && (
                <FilterPopover anchor="年代范围" onClose={() => setOpenFilter(null)}>
                  <YearFilter yearFrom={yearFrom} yearTo={yearTo} onChange={(f, t) => { setYearFrom(f); setYearTo(t); }} />
                </FilterPopover>
              )}
            </div>

            {/* Genres */}
            <div style={{ position: "relative" }}>
              {selectedGenreNames.length > 0
                ? selectedGenreNames.map((name, i) => (
                    <Chip key={genreIds[i]} label={name} active
                      onRemove={() => setGenreIds((ids) => ids.filter((id) => id !== genreIds[i]))} />
                  ))
                : <Chip label="类型" onClick={() => setOpenFilter(openFilter === "genre" ? null : "genre")} />
              }
              {openFilter === "genre" && (
                <FilterPopover anchor="选择类型" onClose={() => setOpenFilter(null)}>
                  <GenreFilter genres={genres} selected={genreIds} onChange={(ids) => { setGenreIds(ids); setOpenFilter(null); }} />
                </FilterPopover>
              )}
            </div>

            {/* Rating */}
            <div style={{ position: "relative" }}>
              <Chip
                label={minRating ? `≥ ${minRating}` : "评分"}
                active={!!minRating}
                onRemove={minRating ? () => setMinRating(undefined) : undefined}
                onClick={() => setOpenFilter(openFilter === "rating" ? null : "rating")}
              />
              {openFilter === "rating" && (
                <FilterPopover anchor="最低评分" onClose={() => setOpenFilter(null)}>
                  <RatingFilter current={minRating} onChange={(v) => { setMinRating(v); setOpenFilter(null); }} />
                </FilterPopover>
              )}
            </div>

            {/* Sort */}
            <div style={{ position: "relative" }}>
              <Chip
                label={SORT_OPTIONS.find((o) => o.value === sortBy)?.label ?? "排序"}
                active
                onClick={() => setOpenFilter(openFilter === "sort" ? null : "sort")}
              />
              {openFilter === "sort" && (
                <FilterPopover anchor="排序方式" onClose={() => setOpenFilter(null)}>
                  <SortFilter current={sortBy} onChange={(v) => { setSortBy(v); setOpenFilter(null); }} />
                </FilterPopover>
              )}
            </div>
          </div>
        </div>

        {/* Divider */}
        <div style={{ height: 1, background: "var(--color-separator)", margin: "0 1.5rem" }} />

        {/* Results */}
        <div style={{ flex: 1, overflowY: "auto", padding: "0.9rem 1.5rem" }}>
          {!apiKey && (
            <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>
              请先在设置中配置 TMDB API Key。
            </p>
          )}

          {results.map((m) => (
            <div
              key={m.id}
              onClick={() => setSelected(m)}
              style={{
                display: "flex",
                gap: "0.75rem",
                padding: "0.65rem 0.5rem",
                borderRadius: 6,
                cursor: "pointer",
                background: selected?.id === m.id ? "var(--color-bg-elevated)" : "transparent",
              }}
              onMouseEnter={(e) => {
                if (selected?.id !== m.id) (e.currentTarget as HTMLDivElement).style.background = "var(--color-hover)";
              }}
              onMouseLeave={(e) => {
                if (selected?.id !== m.id) (e.currentTarget as HTMLDivElement).style.background = "transparent";
              }}
            >
              {m.poster_path && (
                <img
                  src={`https://image.tmdb.org/t/p/w92${m.poster_path}`}
                  alt=""
                  style={{ width: 46, height: 69, borderRadius: 4, objectFit: "cover", flexShrink: 0 }}
                />
              )}
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: "0.85rem", fontWeight: 500, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                  {m.title}
                </div>
                <div style={{ fontSize: "0.72rem", color: "var(--color-label-tertiary)", marginTop: "0.15rem" }}>
                  {m.year ?? "—"}
                  {m.vote_average > 0 && (
                    <span style={{ marginLeft: "0.5rem", color: "var(--color-accent)" }}>
                      ★ {m.vote_average.toFixed(1)}
                    </span>
                  )}
                </div>
                {m.director && (
                  <div style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)", marginTop: "0.15rem", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                    导演: {m.director}
                  </div>
                )}
                {m.cast && m.cast.length > 0 && (
                  <div style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)", marginTop: "0.1rem", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                    主演: {m.cast.join(", ")}
                  </div>
                )}
              </div>
            </div>
          ))}

          {hasMore && !loading && (
            <button onClick={loadMore}
              style={{
                display: "block", margin: "0.75rem auto", background: "none",
                border: "1px solid var(--color-separator)", borderRadius: 6,
                padding: "0.4rem 1.2rem", color: "var(--color-label-secondary)",
                cursor: "pointer", fontSize: "0.78rem", fontFamily: "inherit",
              }}>
              加载更多
            </button>
          )}

          {loading && <p style={{ textAlign: "center", color: "var(--color-label-tertiary)", fontSize: "0.78rem" }}>搜索中…</p>}
        </div>
      </div>

      {/* Right panel */}
      {selected && (
        <div
          style={{
            width: 380,
            flexShrink: 0,
            borderLeft: "1px solid var(--color-separator)",
            overflowY: "auto",
          }}
        >
          <FilmDetailPanel film={selected} onClose={() => setSelected(null)} />
        </div>
      )}
    </div>
  );
}
