// src/components/FilmDetailPanel.tsx
import { useState, useEffect } from "react";
import { yts, download } from "../lib/tauri";
import { formatSize } from "../lib/format";
import type { MovieListItem, MovieResult, TorrentFileInfo } from "../lib/tauri";

function ensurePulseAnimation() {
  if (!document.head.querySelector("[data-blowup-pulse]")) {
    const style = document.createElement("style");
    style.textContent = `@keyframes pulse{0%,100%{opacity:.2;transform:scale(.8)}50%{opacity:1;transform:scale(1.2)}}`;
    style.setAttribute("data-blowup-pulse", "");
    document.head.appendChild(style);
  }
}

function TorrentSearchModal({
  film,
  onClose,
}: {
  film: MovieListItem;
  onClose: () => void;
}) {
  const [results, setResults] = useState<MovieResult[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [fetching, setFetching] = useState<Set<string>>(new Set());
  const [started, setStarted] = useState<Set<string>>(new Set());
  // File selection modal state
  const [filePickResult, setFilePickResult] = useState<MovieResult | null>(null);
  const [fileList, setFileList] = useState<TorrentFileInfo[]>([]);
  const [selectedFiles, setSelectedFiles] = useState<Set<number>>(new Set());
  const [submitting, setSubmitting] = useState(false);

  const year = film.year ? parseInt(film.year) : undefined;

  useEffect(() => { ensurePulseAnimation(); }, []);

  useEffect(() => {
    yts
      .search(film.title, year, film.id)
      .then(setResults)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [film.title, year, film.id]);

  // Step 1: fetch torrent file list
  const handleFetchFiles = async (r: MovieResult) => {
    const target = r.magnet ?? r.torrent_url;
    if (!target) return;
    setFetching((prev) => new Set(prev).add(target));
    try {
      const files = await download.getTorrentFiles(target);
      setFileList(files);
      setSelectedFiles(new Set(files.map((f) => f.index)));
      setFilePickResult(r);
    } catch (e) {
      console.error("fetch torrent files failed:", e);
    } finally {
      setFetching((prev) => { const next = new Set(prev); next.delete(target); return next; });
    }
  };

  // Step 2: confirm and start download
  const handleConfirmDownload = async () => {
    if (!filePickResult) return;
    const target = filePickResult.magnet ?? filePickResult.torrent_url;
    if (!target) return;
    setSubmitting(true);
    try {
      await download.startDownload({
        title: film.title,
        target,
        director: film.director ?? "Unknown",
        tmdbId: film.id,
        year: year,
        genres: [],
        quality: filePickResult.quality,
        onlyFiles: [...selectedFiles],
      });
      setStarted((prev) => new Set(prev).add(target));
      setFilePickResult(null);
    } catch (e) {
      console.error("download failed:", e);
    } finally {
      setSubmitting(false);
    }
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
        <h3 style={{ margin: "0 0 12px" }}>搜索资源: {film.title}</h3>

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
                  下载中
                </span>
              ) : fetching.has(target) ? (
                <span style={{ fontSize: 12, display: "inline-flex", gap: 3 }}>
                  {[0, 1, 2].map((j) => (
                    <span key={j} style={{
                      width: 4, height: 4, borderRadius: "50%", background: "var(--color-accent)",
                      animation: "pulse 1.2s ease-in-out infinite",
                      animationDelay: `${j * 0.2}s`,
                    }} />
                  ))}
                </span>
              ) : (
                <button
                  onClick={() => handleFetchFiles(r)}
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

      {/* File selection modal */}
      {filePickResult && (
        <div
          onClick={() => !submitting && setFilePickResult(null)}
          style={{
            position: "fixed",
            inset: 0,
            background: "rgba(0,0,0,0.5)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1100,
          }}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              background: "var(--color-bg-primary)",
              borderRadius: 12,
              padding: 24,
              width: 520,
              maxHeight: "70vh",
              display: "flex",
              flexDirection: "column",
            }}
          >
            <h3 style={{ margin: "0 0 4px" }}>选择下载文件</h3>
            <p style={{ margin: "0 0 12px", fontSize: 12, color: "var(--color-label-secondary)" }}>
              {filePickResult.quality} · 共 {fileList.length} 个文件
            </p>

            <div style={{ flex: 1, overflowY: "auto", marginBottom: 16 }}>
              {fileList.map((f) => (
                <label
                  key={f.index}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    padding: "6px 0",
                    borderBottom: "1px solid var(--color-separator)",
                    fontSize: 13,
                    cursor: "pointer",
                  }}
                >
                  <input
                    type="checkbox"
                    checked={selectedFiles.has(f.index)}
                    onChange={() => {
                      setSelectedFiles((prev) => {
                        const next = new Set(prev);
                        if (next.has(f.index)) next.delete(f.index);
                        else next.add(f.index);
                        return next;
                      });
                    }}
                  />
                  <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {f.name}
                  </span>
                  <span style={{ color: "var(--color-label-tertiary)", flexShrink: 0 }}>
                    {formatSize(f.size)}
                  </span>
                </label>
              ))}
            </div>

            <div style={{ display: "flex", gap: 8 }}>
              <button
                onClick={() => {
                  if (selectedFiles.size === fileList.length) setSelectedFiles(new Set());
                  else setSelectedFiles(new Set(fileList.map((f) => f.index)));
                }}
                style={{
                  background: "var(--color-bg-control)",
                  border: "1px solid var(--color-separator)",
                  borderRadius: 6,
                  padding: "6px 16px",
                  color: "var(--color-label-primary)",
                  cursor: "pointer",
                  fontSize: 13,
                }}
              >
                {selectedFiles.size === fileList.length ? "取消全选" : "全选"}
              </button>
              <div style={{ flex: 1 }} />
              <button
                onClick={() => setFilePickResult(null)}
                disabled={submitting}
                style={{
                  background: "var(--color-bg-control)",
                  border: "1px solid var(--color-separator)",
                  borderRadius: 6,
                  padding: "6px 16px",
                  color: "var(--color-label-primary)",
                  cursor: "pointer",
                  fontSize: 13,
                }}
              >
                取消
              </button>
              <button
                onClick={handleConfirmDownload}
                disabled={selectedFiles.size === 0 || submitting}
                style={{
                  background: selectedFiles.size === 0 ? "var(--color-bg-control)" : "var(--color-accent)",
                  color: selectedFiles.size === 0 ? "var(--color-label-tertiary)" : "#fff",
                  border: "none",
                  borderRadius: 6,
                  padding: "6px 16px",
                  cursor: selectedFiles.size === 0 ? "not-allowed" : "pointer",
                  fontSize: 13,
                }}
              >
                {submitting ? "开始中..." : `确认下载 (${selectedFiles.size})`}
              </button>
            </div>
          </div>
        </div>
      )}
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
          film={film}
          onClose={() => setShowTorrentModal(false)}
        />
      )}
    </>
  );
}
