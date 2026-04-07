// src/components/FilmDetailPanel.tsx
import { useState, useEffect } from "react";
import { yts, download } from "../lib/tauri";
import type { MovieListItem, MovieResult } from "../lib/tauri";

function TorrentSearchModal({
  title,
  year,
  tmdbId,
  filmId,
  onClose,
}: {
  title: string;
  year?: number;
  tmdbId?: number;
  filmId?: number;
  onClose: () => void;
}) {
  const [results, setResults] = useState<MovieResult[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [started, setStarted] = useState<Set<string>>(new Set());

  useEffect(() => {
    yts
      .search(title, year, tmdbId)
      .then(setResults)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [title, year, tmdbId]);

  const handleDownload = async (r: MovieResult) => {
    const target = r.magnet ?? r.torrent_url;
    if (!target) return;
    await download.startDownload(
      `${r.title} (${r.year})`,
      target,
      r.quality,
      filmId
    );
    setStarted((prev) => new Set(prev).add(target));
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: "var(--color-bg-primary)",
          borderRadius: 12,
          padding: 24,
          width: 480,
          maxHeight: "70vh",
          overflowY: "auto",
        }}
      >
        <h3 style={{ margin: "0 0 12px" }}>搜索资源: {title}</h3>

        {loading && (
          <div style={{ color: "var(--color-label-secondary)", fontSize: 13 }}>
            搜索中...
          </div>
        )}

        {error && (
          <div style={{ color: "#e53935", fontSize: 13 }}>
            {error.includes("NoResults") ? "未找到资源" : `搜索失败: ${error}`}
          </div>
        )}

        {!loading && !error && results.length === 0 && (
          <div style={{ color: "var(--color-label-tertiary)", fontSize: 13 }}>
            未找到资源
          </div>
        )}

        {results.map((r, i) => {
          const target = r.magnet ?? r.torrent_url ?? "";
          const isStarted = started.has(target);
          return (
            <div
              key={i}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "8px 0",
                borderBottom: "1px solid var(--color-separator)",
                fontSize: 13,
              }}
            >
              <div>
                <span style={{ fontWeight: 500 }}>{r.quality}</span>
                <span style={{ color: "var(--color-label-secondary)", marginLeft: 12 }}>
                  {r.seeds} seeds
                </span>
              </div>
              {isStarted ? (
                <span style={{ color: "var(--color-accent)", fontSize: 12 }}>
                  ✓ 已添加
                </span>
              ) : (
                <button
                  onClick={() => handleDownload(r)}
                  disabled={!target}
                  style={{
                    background: "var(--color-accent)",
                    color: "#fff",
                    border: "none",
                    borderRadius: 4,
                    padding: "3px 12px",
                    cursor: "pointer",
                    fontSize: 12,
                  }}
                >
                  下载
                </button>
              )}
            </div>
          );
        })}

        <button
          onClick={onClose}
          style={{
            marginTop: 16,
            background: "var(--color-bg-control)",
            border: "1px solid var(--color-separator)",
            borderRadius: 6,
            padding: "6px 16px",
            color: "var(--color-label-primary)",
            cursor: "pointer",
            fontSize: 13,
            width: "100%",
          }}
        >
          关闭
        </button>
      </div>
    </div>
  );
}

interface FilmDetailPanelProps {
  film: MovieListItem;
  onClose: () => void;
}

export function FilmDetailPanel({ film, onClose }: FilmDetailPanelProps) {
  const [showTorrentModal, setShowTorrentModal] = useState(false);

  return (
    <>
      <div style={{ width: "100%", background: "var(--color-bg-secondary)", overflowY: "auto", padding: "1.25rem 1.25rem 2rem", display: "flex", flexDirection: "column", gap: "0.75rem" }}>
        <div style={{ display: "flex", justifyContent: "flex-end" }}>
          <button onClick={onClose} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "1rem", lineHeight: 1, padding: 0 }}>✕</button>
        </div>

        {film.poster_path && (
          <img src={`https://image.tmdb.org/t/p/w300${film.poster_path}`} alt={film.title} style={{ width: "100%", borderRadius: 6 }} />
        )}

        <div>
          <h2 style={{ margin: 0, fontSize: "1rem", fontWeight: 700, letterSpacing: "-0.02em" }}>{film.title}</h2>
          {film.original_title !== film.title && (
            <p style={{ margin: "0.15rem 0 0", fontSize: "0.72rem", color: "var(--color-label-tertiary)" }}>{film.original_title}</p>
          )}
        </div>

        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          {film.year && <span style={{ fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>{film.year}</span>}
          <span style={{ fontSize: "0.75rem", color: "var(--color-label-tertiary)" }}>·</span>
          <span style={{ fontSize: "0.75rem", color: "var(--color-accent)", fontWeight: 500 }}>★ {film.vote_average.toFixed(1)}</span>
        </div>

        {(film.director || (film.cast && film.cast.length > 0)) && (
          <div style={{ fontSize: "0.75rem", color: "var(--color-label-secondary)", lineHeight: 1.5 }}>
            {film.director && <div>导演: {film.director}</div>}
            {film.cast && film.cast.length > 0 && <div>主演: {film.cast.join(", ")}</div>}
          </div>
        )}

        <p style={{ margin: 0, fontSize: "0.78rem", color: "var(--color-label-secondary)", lineHeight: 1.6 }}>
          {film.overview || "暂无简介。"}
        </p>

        <div style={{ borderTop: "1px solid var(--color-separator)", paddingTop: "0.75rem" }}>
          <button
            onClick={() => setShowTorrentModal(true)}
            style={{
              background: "var(--color-bg-control)",
              border: "1px solid var(--color-separator)",
              borderRadius: 8,
              padding: "0.4rem 0.75rem",
              color: "var(--color-label-primary)",
              cursor: "pointer",
              fontSize: "0.78rem",
            }}
          >
            搜索资源
          </button>
        </div>
      </div>

      {showTorrentModal && (
        <TorrentSearchModal
          title={film.title}
          year={film.year ? parseInt(film.year) : undefined}
          tmdbId={film.id}
          onClose={() => setShowTorrentModal(false)}
        />
      )}
    </>
  );
}
