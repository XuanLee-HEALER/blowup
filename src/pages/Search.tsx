// src/pages/Search.tsx
import { useState, useEffect, useRef, useCallback } from "react";
import { TextInput } from "../components/ui/TextInput";
import { Chip } from "../components/ui/Chip";
import { tmdb, config, type MovieListItem, type TmdbGenre, type SearchFilters } from "../lib/tauri";
import { FilmDetailPanel } from "../components/FilmDetailPanel";

const SORT_OPTIONS = [
  { value: "vote_average.desc", label: "按评分排序" },
  { value: "popularity.desc",   label: "按热度排序" },
  { value: "release_date.desc", label: "按年份排序" },
];

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

  const searchTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // Load API key and genre list on mount
  useEffect(() => {
    config.get().then((cfg) => {
      setApiKey(cfg.tmdb.api_key);
      if (cfg.tmdb.api_key) {
        tmdb.listGenres(cfg.tmdb.api_key).then(setGenres).catch(() => {});
      }
    });
  }, []);

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
        const rows =
          q.trim()
            ? await tmdb.searchMovies(apiKey, q.trim(), filters)
            : await tmdb.discoverMovies(apiKey, filters);

        setResults((prev) => (append ? [...prev, ...rows] : rows));
        setHasMore(rows.length === 20);
        setPage(p);
      } catch (e) {
        console.error(e);
      } finally {
        setLoading(false);
      }
    },
    [apiKey, buildFilters]
  );

  // Debounced search on query / filter change
  useEffect(() => {
    clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => runSearch(query, 1, false), 400);
    return () => clearTimeout(searchTimer.current);
  }, [query, yearFrom, yearTo, genreIds, minRating, sortBy, apiKey, runSearch]);

  const loadMore = () => runSearch(query, page + 1, true);

  // Selected genre names for chip display
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
            placeholder="电影名称、导演…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            style={{ marginBottom: "0.7rem" }}
          />

          {/* Filter chips */}
          <div style={{ display: "flex", gap: "0.4rem", flexWrap: "wrap", marginBottom: "1rem" }}>
            {/* Year range */}
            <Chip
              label={yearFrom || yearTo ? `${yearFrom ?? "?"} – ${yearTo ?? "?"}` : "年代"}
              active={!!(yearFrom || yearTo)}
              onRemove={
                yearFrom || yearTo
                  ? () => { setYearFrom(undefined); setYearTo(undefined); }
                  : undefined
              }
              onClick={() => {
                const from = prompt("起始年份 (留空跳过)");
                const to   = prompt("结束年份 (留空跳过)");
                setYearFrom(from ? parseInt(from) : undefined);
                setYearTo(to   ? parseInt(to)   : undefined);
              }}
            />

            {/* Genres */}
            {selectedGenreNames.length > 0
              ? selectedGenreNames.map((name, i) => (
                  <Chip
                    key={genreIds[i]}
                    label={name}
                    active
                    onRemove={() =>
                      setGenreIds((ids) => ids.filter((id) => id !== genreIds[i]))
                    }
                  />
                ))
              : (
                <Chip
                  label="类型"
                  onClick={() => {
                    const names = genres.map((g, i) => `${i + 1}. ${g.name}`).join("\n");
                    const pick = prompt(`选择类型序号（逗号分隔）:\n${names}`);
                    if (pick) {
                      const ids = pick.split(",")
                        .map((s) => genres[parseInt(s.trim()) - 1]?.id)
                        .filter(Boolean) as number[];
                      setGenreIds(ids);
                    }
                  }}
                />
              )}

            {/* Rating */}
            <Chip
              label={minRating ? `≥ ${minRating}` : "评分"}
              active={!!minRating}
              onRemove={minRating ? () => setMinRating(undefined) : undefined}
              onClick={() => {
                const r = prompt("最低评分 (0–10)");
                setMinRating(r ? parseFloat(r) : undefined);
              }}
            />

            {/* Sort */}
            <Chip
              label={SORT_OPTIONS.find((o) => o.value === sortBy)?.label ?? "排序"}
              active
              onClick={() => {
                const opts = SORT_OPTIONS.map((o, i) => `${i + 1}. ${o.label}`).join("\n");
                const pick = prompt(`排序方式:\n${opts}`);
                if (pick) {
                  const opt = SORT_OPTIONS[parseInt(pick) - 1];
                  if (opt) setSortBy(opt.value);
                }
              }}
            />
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

          {results.map((film) => (
            <FilmRow
              key={film.id}
              film={film}
              selected={selected?.id === film.id}
              onClick={() => setSelected(film)}
            />
          ))}

          {loading && (
            <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.75rem", padding: "0.5rem 0" }}>
              加载中…
            </p>
          )}

          {hasMore && !loading && (
            <button
              onClick={loadMore}
              style={{
                background: "none",
                border: "none",
                color: "var(--color-label-tertiary)",
                fontSize: "0.75rem",
                cursor: "pointer",
                padding: "0.5rem 0",
                fontFamily: "inherit",
              }}
            >
              加载更多
            </button>
          )}
        </div>
      </div>

      {/* Right: detail panel */}
      {selected && (
        <FilmDetailPanel film={selected} onClose={() => setSelected(null)} />
      )}
    </div>
  );
}

// ── FilmRow ──────────────────────────────────────────────────────
function FilmRow({
  film,
  selected,
  onClick,
}: {
  film: MovieListItem;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <div
      onClick={onClick}
      style={{
        display: "flex",
        alignItems: "center",
        gap: "0.85rem",
        padding: "0.55rem 0.6rem",
        borderRadius: "7px",
        cursor: "pointer",
        background: selected ? "var(--color-bg-elevated)" : "transparent",
      }}
      onMouseEnter={(e) => {
        if (!selected)
          (e.currentTarget as HTMLDivElement).style.background =
            "rgba(255,255,255,0.04)";
      }}
      onMouseLeave={(e) => {
        if (!selected)
          (e.currentTarget as HTMLDivElement).style.background = "transparent";
      }}
    >
      {/* Poster */}
      <div
        style={{
          width: 34,
          height: 48,
          background: "var(--color-bg-elevated)",
          borderRadius: 3,
          flexShrink: 0,
          overflow: "hidden",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--color-label-quaternary)",
          fontSize: "0.9rem",
        }}
      >
        {film.poster_path ? (
          <img
            src={`https://image.tmdb.org/t/p/w92${film.poster_path}`}
            alt=""
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          "🎬"
        )}
      </div>

      {/* Info */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <p
          style={{
            margin: 0,
            fontSize: "0.86rem",
            fontWeight: 500,
            letterSpacing: "-0.01em",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {film.title}
        </p>
        <p
          style={{
            margin: "0.12rem 0 0",
            fontSize: "0.7rem",
            color: "var(--color-label-tertiary)",
          }}
        >
          {[film.year].filter(Boolean).join(" · ")}
        </p>
      </div>

      {/* Score */}
      <span style={{ fontSize: "0.72rem", color: "var(--color-label-tertiary)", flexShrink: 0 }}>
        <strong style={{ color: "var(--color-label-secondary)", fontWeight: 500, fontSize: "0.8rem" }}>
          {film.vote_average.toFixed(1)}
        </strong>{" "}
        / 10
      </span>
    </div>
  );
}
