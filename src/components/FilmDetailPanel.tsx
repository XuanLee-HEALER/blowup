// src/components/FilmDetailPanel.tsx
import { useState, useEffect } from "react";
import { tmdb, library, config, yts, download } from "../lib/tauri";
import type { MovieListItem, TmdbPersonInput, TmdbMovieInput, MovieResult } from "../lib/tauri";

const JOB_TO_ROLE: Record<string, string> = {
  "Director": "director",
  "Director of Photography": "cinematographer",
  "Original Music Composer": "composer",
  "Editor": "editor",
  "Screenplay": "screenwriter",
  "Writer": "screenwriter",
  "Producer": "producer",
};

const ROLE_OPTIONS = ["director", "cinematographer", "composer", "editor", "screenwriter", "producer", "actor"];

interface PersonSel {
  tmdbId: number | null;
  name: string;
  job: string;
  primary_role: string;
  selected: boolean;
}

function PersonRow({ person, onToggle, onRoleChange }: { person: PersonSel; onToggle: () => void; onRoleChange: (r: string) => void }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.6rem", padding: "0.3rem 0", fontSize: "0.8rem" }}>
      <input type="checkbox" checked={person.selected} onChange={onToggle} style={{ cursor: "pointer", accentColor: "var(--color-accent)" }} />
      <span style={{ flex: 1, color: person.selected ? "var(--color-label-primary)" : "var(--color-label-tertiary)" }}>{person.name}</span>
      <span style={{ color: "var(--color-label-quaternary)", fontSize: "0.7rem" }}>{person.job}</span>
      {person.selected && (
        <select value={person.primary_role} onChange={(e) => onRoleChange(e.target.value)}
          style={{ background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.15rem 0.3rem", color: "var(--color-label-secondary)", fontSize: "0.7rem", fontFamily: "inherit", cursor: "pointer" }}>
          {ROLE_OPTIONS.map((r) => <option key={r} value={r}>{r}</option>)}
        </select>
      )}
    </div>
  );
}

function AddToLibraryModal({ film, apiKey, onClose, onAdded }: { film: MovieListItem; apiKey: string; onClose: () => void; onAdded: () => void }) {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [crew, setCrew] = useState<PersonSel[]>([]);
  const [cast, setCast] = useState<PersonSel[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    tmdb.getMovieCredits(apiKey, film.id).then((credits) => {
      setCrew(credits.crew.map((m) => ({ tmdbId: m.id, name: m.name, job: m.job, primary_role: JOB_TO_ROLE[m.job] ?? "director", selected: m.job === "Director" })));
      setCast(credits.cast.map((m) => ({ tmdbId: m.id, name: m.name, job: m.character, primary_role: "actor", selected: false })));
      setLoading(false);
    }).catch((e) => { setError(String(e)); setLoading(false); });
  }, []);

  const confirm = async () => {
    setSaving(true);
    const people: TmdbPersonInput[] = [
      ...crew.filter((p) => p.selected).map((p) => ({ tmdb_id: p.tmdbId, name: p.name, role: JOB_TO_ROLE[p.job] ?? "director", primary_role: p.primary_role })),
      ...cast.filter((p) => p.selected).map((p) => ({ tmdb_id: p.tmdbId, name: p.name, role: "actor", primary_role: "actor" })),
    ];

    const input: TmdbMovieInput = {
      tmdb_id: film.id, title: film.title,
      original_title: film.original_title !== film.title ? film.original_title : null,
      year: film.year ? parseInt(film.year) : null,
      overview: film.overview || null, tmdb_rating: film.vote_average, people,
    };

    try {
      await library.addFilmFromTmdb(input);
      onAdded(); onClose();
    } catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  };

  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.6)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 100 }} onClick={onClose}>
      <div style={{ background: "var(--color-bg-secondary)", border: "1px solid var(--color-separator)", borderRadius: 10, padding: "1.5rem", width: 460, maxHeight: "80vh", overflowY: "auto", display: "flex", flexDirection: "column", gap: "1rem" }} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ margin: 0, fontSize: "1rem", fontWeight: 700 }}>加入知识库</h3>
        <div>
          <p style={{ margin: "0 0 0.15rem", fontWeight: 600 }}>{film.title}</p>
          {film.year && <p style={{ margin: 0, fontSize: "0.75rem", color: "var(--color-label-tertiary)" }}>{film.year}</p>}
        </div>
        {loading && <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.8rem" }}>加载演职员…</p>}
        {error && <p style={{ color: "#e57373", fontSize: "0.8rem" }}>{error}</p>}
        {!loading && !error && (
          <>
            {crew.length > 0 && (
              <div>
                <p style={{ margin: "0 0 0.4rem", fontSize: "0.72rem", color: "var(--color-label-quaternary)", textTransform: "uppercase" }}>主创</p>
                {crew.map((p, i) => (
                  <PersonRow key={`c${i}`} person={p}
                    onToggle={() => setCrew((prev) => prev.map((x, xi) => xi === i ? { ...x, selected: !x.selected } : x))}
                    onRoleChange={(r) => setCrew((prev) => prev.map((x, xi) => xi === i ? { ...x, primary_role: r } : x))}
                  />
                ))}
              </div>
            )}
            {cast.length > 0 && (
              <div>
                <p style={{ margin: "0 0 0.4rem", fontSize: "0.72rem", color: "var(--color-label-quaternary)", textTransform: "uppercase" }}>演员（前5位）</p>
                {cast.map((p, i) => (
                  <PersonRow key={`a${i}`} person={p}
                    onToggle={() => setCast((prev) => prev.map((x, xi) => xi === i ? { ...x, selected: !x.selected } : x))}
                    onRoleChange={(r) => setCast((prev) => prev.map((x, xi) => xi === i ? { ...x, primary_role: r } : x))}
                  />
                ))}
              </div>
            )}
          </>
        )}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem" }}>
          <button onClick={onClose} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>取消</button>
          <button onClick={confirm} disabled={saving || loading}
            style={{ background: "var(--color-accent)", border: "none", borderRadius: 6, padding: "0.35rem 1rem", color: "#0B1628", fontWeight: 600, cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>
            {saving ? "保存中…" : "确认加入"}
          </button>
        </div>
      </div>
    </div>
  );
}

function TorrentSearchModal({
  title,
  filmId,
  onClose,
}: {
  title: string;
  filmId?: number;
  onClose: () => void;
}) {
  const [results, setResults] = useState<MovieResult[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [started, setStarted] = useState<Set<string>>(new Set());

  useEffect(() => {
    yts
      .search(title)
      .then(setResults)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [title]);

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
  const [apiKey, setApiKey] = useState("");
  const [showAddModal, setShowAddModal] = useState(false);
  const [addedToLibrary, setAddedToLibrary] = useState(false);
  const [showTorrentModal, setShowTorrentModal] = useState(false);

  useEffect(() => { config.get().then((cfg) => setApiKey(cfg.tmdb.api_key)); }, []);

  return (
    <>
      <div style={{ width: 300, flexShrink: 0, borderLeft: "1px solid var(--color-separator)", background: "var(--color-bg-secondary)", overflowY: "auto", padding: "1.25rem 1.25rem 2rem", display: "flex", flexDirection: "column", gap: "0.75rem" }}>
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

        <p style={{ margin: 0, fontSize: "0.78rem", color: "var(--color-label-secondary)", lineHeight: 1.6 }}>
          {film.overview || "暂无简介。"}
        </p>

        <div style={{ borderTop: "1px solid var(--color-separator)", paddingTop: "0.75rem", display: "flex", flexDirection: "column", gap: "0.4rem" }}>
          {addedToLibrary ? (
            <p style={{ fontSize: "0.78rem", color: "var(--color-accent)", margin: 0 }}>✓ 已加入知识库</p>
          ) : (
            <button onClick={() => setShowAddModal(true)}
              style={{ background: "var(--color-accent-soft)", border: "1px solid var(--color-accent)", borderRadius: 6, padding: "0.4rem 0.75rem", color: "var(--color-accent)", fontSize: "0.78rem", cursor: "pointer", fontFamily: "inherit", textAlign: "left" }}>
              加入知识库
            </button>
          )}
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

      {showAddModal && (
        <AddToLibraryModal film={film} apiKey={apiKey}
          onClose={() => setShowAddModal(false)}
          onAdded={() => setAddedToLibrary(true)}
        />
      )}

      {showTorrentModal && (
        <TorrentSearchModal
          title={film.title}
          filmId={undefined}
          onClose={() => setShowTorrentModal(false)}
        />
      )}
    </>
  );
}
