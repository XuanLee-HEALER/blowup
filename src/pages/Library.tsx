import { useState, useEffect, useCallback } from "react";
import { library, media, config, player } from "../lib/tauri";
import type { IndexEntry } from "../lib/tauri";
import { TextInput } from "../components/ui/TextInput";

const VIDEO_EXTS = ["mp4", "mkv", "avi", "mov", "ts", "flv", "wmv", "webm", "m4v"];
const SUB_EXTS = ["srt", "ass", "sub", "idx"];
const getExt = (f: string) => f.split(".").pop()?.toLowerCase() ?? "";

export default function Library() {
  const [directorMap, setDirectorMap] = useState<Record<string, IndexEntry[]>>({});
  const [selectedDirector, setSelectedDirector] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<IndexEntry | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<IndexEntry[] | null>(null);
  const [enriching, setEnriching] = useState(false);
  const [checkedSubs, setCheckedSubs] = useState<Set<string>>(new Set());
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; entry: IndexEntry } | null>(null);

  const refresh = useCallback(async () => {
    const map = await library.listIndexByDirector();
    setDirectorMap(map);
  }, []);

  useEffect(() => { library.listIndexByDirector().then(setDirectorMap); }, []);

  // Select entry and trigger TMDB enrichment if needed
  const selectEntry = useCallback((entry: IndexEntry) => {
    setSelectedEntry(entry);
    setCheckedSubs(new Set());
    if (!entry.poster_url) {
      setEnriching(true);
      library.enrichIndexEntry(entry.tmdb_id)
        .then((enriched) => setSelectedEntry(enriched))
        .catch(() => { /* TMDB unavailable, show basic info */ })
        .finally(() => setEnriching(false));
    }
  }, []);

  const doSearch = async () => {
    if (!searchQuery.trim()) { setSearchResults(null); return; }
    const results = await library.searchIndex(searchQuery.trim());
    setSearchResults(results);
  };

  const handlePlay = async (entry: IndexEntry, file: string) => {
    const cfg = await config.get();
    const root = cfg.library.root_dir;
    const fullPath = `${root}/${entry.path}/${file}`;
    await media.openInPlayer(fullPath);
    // Auto-load checked subtitles after player initializes
    if (checkedSubs.size > 0) {
      setTimeout(async () => {
        for (const sub of checkedSubs) {
          const subPath = `${root}/${entry.path}/${sub}`;
          try { await player.subAdd(subPath); } catch { /* ignore */ }
        }
      }, 500);
    }
  };

  const handleDeleteResource = async (entry: IndexEntry, file: string) => {
    if (!confirm(`确定要删除文件 "${file}" 吗？此操作不可撤销。`)) return;
    try {
      const cfg = await config.get();
      const fullPath = `${cfg.library.root_dir}/${entry.path}/${file}`;
      await library.deleteLibraryResource(fullPath);
      await library.refreshIndexEntry(entry.tmdb_id);
      const map = await library.listIndexByDirector();
      setDirectorMap(map);
      // Update selected entry
      for (const entries of Object.values(map)) {
        const updated = entries.find((e) => e.tmdb_id === entry.tmdb_id);
        if (updated) { setSelectedEntry(updated); return; }
      }
      setSelectedEntry(null);
    } catch (e) { alert(`删除失败: ${e}`); }
  };

  const handleDeleteFilm = async (entry: IndexEntry) => {
    setContextMenu(null);
    if (!confirm(`确定要删除电影 "${entry.title}" 及其所有文件吗？此操作不可撤销。`)) return;
    try {
      await library.deleteFilmDirectory(entry.tmdb_id);
      await refresh();
      if (selectedEntry?.tmdb_id === entry.tmdb_id) {
        setSelectedEntry(null);
      }
    } catch (e) { alert(`删除失败: ${e}`); }
  };

  const handleRefreshDetail = async () => {
    if (!selectedEntry) return;
    setEnriching(true);
    library.enrichIndexEntry(selectedEntry.tmdb_id, true)
      .then((enriched) => setSelectedEntry(enriched))
      .catch((e) => alert(`刷新失败: ${e}`))
      .finally(() => setEnriching(false));
  };

  const handleRebuild = async () => {
    await library.rebuildIndex();
    await refresh();
  };

  const toggleSub = (file: string) => {
    setCheckedSubs((prev) => {
      const next = new Set(prev);
      if (next.has(file)) next.delete(file); else next.add(file);
      return next;
    });
  };

  const directors = Object.keys(directorMap).sort();
  const videoFiles = selectedEntry?.files.filter((f) => VIDEO_EXTS.includes(getExt(f))) ?? [];
  const subtitleFiles = selectedEntry?.files.filter((f) => SUB_EXTS.includes(getExt(f))) ?? [];

  // Credits display order
  const CREDIT_ORDER = ["导演", "主演", "编剧", "摄影", "配乐", "剪辑", "制片"];
  const credits = selectedEntry?.credits ?? {};

  const onEntryContextMenu = (e: React.MouseEvent, entry: IndexEntry) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, entry });
  };

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      {/* ── Left: director list + search ── */}
      <div style={{ width: 240, flexShrink: 0, borderRight: "1px solid var(--color-separator)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
        <div style={{ padding: "1.4rem 1rem 0" }}>
          <h1 style={{ fontSize: "1.3rem", fontWeight: 700, letterSpacing: "-0.035em", marginBottom: "0.8rem" }}>
            电影库
          </h1>
          <TextInput
            leadingIcon="⌕"
            placeholder="搜索标题或导演…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.nativeEvent.isComposing) doSearch(); }}
            style={{ marginBottom: "0.5rem" }}
          />
          <div style={{ display: "flex", gap: "0.3rem", marginBottom: "0.6rem" }}>
            <button onClick={handleRebuild} style={smallBtnStyle}>重建索引</button>
            {searchResults && (
              <button onClick={() => { setSearchResults(null); setSearchQuery(""); }} style={smallBtnStyle}>清除搜索</button>
            )}
          </div>
        </div>

        <div style={{ height: 1, background: "var(--color-separator)", margin: "0 1rem" }} />

        <div style={{ flex: 1, overflowY: "auto", padding: "0.5rem 0", userSelect: "none", WebkitUserSelect: "none" }}>
          {searchResults ? (
            searchResults.map((e) => (
              <div
                key={e.tmdb_id}
                onClick={() => selectEntry(e)}
                onContextMenu={(ev) => onEntryContextMenu(ev, e)}
                style={{
                  padding: "0.5rem 1rem", cursor: "pointer", fontSize: "0.82rem",
                  background: selectedEntry?.tmdb_id === e.tmdb_id ? "var(--color-bg-elevated)" : "transparent",
                }}
              >
                <div style={{ fontWeight: 500 }}>{e.title}</div>
                <div style={{ fontSize: "0.7rem", color: "var(--color-label-tertiary)" }}>
                  {e.director_display} · {e.year ?? "—"}
                </div>
              </div>
            ))
          ) : (
            directors.map((dir) => (
              <div key={dir}>
                <div
                  onClick={() => { setSelectedDirector(selectedDirector === dir ? null : dir); setSelectedEntry(null); setCheckedSubs(new Set()); }}
                  style={{
                    padding: "0.45rem 1rem", cursor: "pointer", fontSize: "0.82rem",
                    fontWeight: 600, display: "flex", justifyContent: "space-between",
                    background: selectedDirector === dir ? "var(--color-bg-elevated)" : "transparent",
                  }}
                >
                  <span>{dir}</span>
                  <span style={{ fontSize: "0.7rem", color: "var(--color-label-quaternary)" }}>
                    {directorMap[dir].length}
                  </span>
                </div>
                {selectedDirector === dir && directorMap[dir].map((e) => (
                  <div
                    key={e.tmdb_id}
                    onClick={() => selectEntry(e)}
                    onContextMenu={(ev) => onEntryContextMenu(ev, e)}
                    style={{
                      padding: "0.35rem 1rem 0.35rem 1.8rem", cursor: "pointer", fontSize: "0.78rem",
                      background: selectedEntry?.tmdb_id === e.tmdb_id ? "rgba(255,255,255,0.06)" : "transparent",
                    }}
                  >
                    <span>{e.title}</span>
                    <span style={{ marginLeft: "0.4rem", fontSize: "0.68rem", color: "var(--color-label-quaternary)" }}>
                      {e.year ?? ""}
                    </span>
                  </div>
                ))}
              </div>
            ))
          )}
          {!searchResults && directors.length === 0 && (
            <p style={{ padding: "1rem", color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>
              电影库为空。通过搜索页下载电影后会自动添加到此处。
            </p>
          )}
        </div>
      </div>

      {/* ── Right: detail panel ── */}
      <div style={{ flex: 1, overflowY: "auto", padding: "1.4rem 1.75rem" }}>
        {selectedEntry ? (
          <div>
            {/* Refresh button */}
            <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: "0.5rem" }}>
              <button
                onClick={handleRefreshDetail}
                disabled={enriching}
                style={{
                  ...smallBtnStyle,
                  opacity: enriching ? 0.5 : 1,
                }}
              >
                {enriching ? "加载中…" : "↻ 刷新信息"}
              </button>
            </div>
            {/* ── Section 1: Film info (poster left, credits right) ── */}
            <div style={{ display: "flex", gap: "1.2rem", marginBottom: "1.5rem" }}>
              {/* Poster */}
              {selectedEntry.poster_url ? (
                <img
                  src={selectedEntry.poster_url.startsWith("http") ? selectedEntry.poster_url : `asset://localhost/${selectedEntry.poster_url}`}
                  alt=""
                  style={{ width: 140, borderRadius: 8, objectFit: "cover", flexShrink: 0, background: "var(--color-bg-secondary)" }}
                />
              ) : (
                <div style={{
                  width: 140, height: 200, borderRadius: 8, flexShrink: 0,
                  background: "var(--color-bg-secondary)", display: "flex",
                  alignItems: "center", justifyContent: "center",
                  color: "var(--color-label-quaternary)", fontSize: "0.75rem",
                }}>
                  {enriching ? "加载中…" : "无海报"}
                </div>
              )}
              {/* Credits */}
              <div style={{ flex: 1, minWidth: 0 }}>
                <h2 style={{ margin: "0 0 0.2rem", fontSize: "1.2rem", fontWeight: 700, letterSpacing: "-0.02em" }}>
                  {selectedEntry.title}
                </h2>
                {selectedEntry.original_title && selectedEntry.original_title !== selectedEntry.title && (
                  <div style={{ fontSize: "0.78rem", color: "var(--color-label-tertiary)", marginBottom: "0.3rem" }}>
                    {selectedEntry.original_title}
                  </div>
                )}
                <div style={{ fontSize: "0.82rem", color: "var(--color-label-secondary)", marginBottom: "0.6rem" }}>
                  {selectedEntry.year ?? "—"}
                  {selectedEntry.rating != null && <span> · ★ {selectedEntry.rating.toFixed(1)}</span>}
                </div>
                {/* Credits: show each role that has data */}
                {Object.keys(credits).length > 0 ? (
                  CREDIT_ORDER
                    .filter((role) => credits[role]?.length)
                    .map((role) => (
                      <div key={role} style={{ fontSize: "0.82rem", marginBottom: "0.3rem" }}>
                        <span style={{ color: "var(--color-label-tertiary)" }}>{role}: </span>
                        {credits[role].join(", ")}
                      </div>
                    ))
                ) : (
                  selectedEntry.director_display && (
                    <div style={{ fontSize: "0.82rem", marginBottom: "0.3rem" }}>
                      <span style={{ color: "var(--color-label-tertiary)" }}>导演: </span>
                      {selectedEntry.director_display}
                    </div>
                  )
                )}
                {selectedEntry.genres.length > 0 && (
                  <div style={{ fontSize: "0.78rem", color: "var(--color-label-tertiary)", marginTop: "0.4rem" }}>
                    {selectedEntry.genres.join(" / ")}
                  </div>
                )}
              </div>
            </div>

            {/* ── Section 2: Video files ── */}
            <SectionHeader>视频文件</SectionHeader>
            {videoFiles.length === 0 ? (
              <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>无视频文件</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem", marginBottom: "1.2rem" }}>
                {videoFiles.map((file) => (
                  <div
                    key={file}
                    style={{
                      display: "flex", alignItems: "center", justifyContent: "space-between",
                      padding: "0.5rem 0.75rem", background: "var(--color-bg-secondary)",
                      border: "1px solid var(--color-separator)", borderRadius: 6,
                    }}
                  >
                    <span style={{ fontSize: "0.82rem", flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {file}
                    </span>
                    <div style={{ display: "flex", gap: "0.3rem", flexShrink: 0, marginLeft: "0.5rem" }}>
                      <button onClick={() => handlePlay(selectedEntry, file)} style={accentBtnStyle}>
                        ▶ 播放
                      </button>
                      <button onClick={() => handleDeleteResource(selectedEntry, file)} style={dangerBtnStyle}>
                        ✕
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* ── Section 3: Subtitle library ── */}
            <SectionHeader>字幕文件</SectionHeader>
            {subtitleFiles.length === 0 ? (
              <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>目录中未发现字幕文件</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem", marginBottom: "1.2rem" }}>
                {subtitleFiles.map((file) => (
                  <label
                    key={file}
                    style={{
                      display: "flex", alignItems: "center", gap: "0.5rem",
                      padding: "0.4rem 0.75rem", background: "var(--color-bg-secondary)",
                      border: "1px solid var(--color-separator)", borderRadius: 6,
                      cursor: "pointer", fontSize: "0.82rem",
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={checkedSubs.has(file)}
                      onChange={() => toggleSub(file)}
                      style={{ accentColor: "var(--color-accent)" }}
                    />
                    <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {file}
                    </span>
                  </label>
                ))}
                {checkedSubs.size > 0 && (
                  <div style={{ fontSize: "0.72rem", color: "var(--color-label-tertiary)", marginTop: "0.2rem" }}>
                    已选 {checkedSubs.size} 条字幕，播放视频时将自动载入
                  </div>
                )}
              </div>
            )}

            {/* Meta */}
            <div style={{ marginTop: "0.5rem", fontSize: "0.7rem", color: "var(--color-label-quaternary)" }}>
              路径: {selectedEntry.path}
              <br />
              添加时间: {new Date(selectedEntry.added_at).toLocaleString("zh-CN")}
            </div>
          </div>
        ) : (
          <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>
            {directors.length > 0 ? "选择一部电影查看详情" : ""}
          </div>
        )}
      </div>

      {/* ── Context menu overlay ── */}
      {contextMenu && (
        <>
          <div
            style={{ position: "fixed", inset: 0, zIndex: 999 }}
            onClick={() => setContextMenu(null)}
            onContextMenu={(e) => { e.preventDefault(); setContextMenu(null); }}
          />
          <div style={{
            position: "fixed", left: contextMenu.x, top: contextMenu.y, zIndex: 1000,
            background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)",
            borderRadius: 6, padding: "4px 0", boxShadow: "0 4px 16px rgba(0,0,0,0.35)", minWidth: 120,
          }}>
            <div
              onClick={() => handleDeleteFilm(contextMenu.entry)}
              style={{
                padding: "6px 14px", fontSize: "0.78rem", cursor: "pointer", color: "#e53935",
              }}
              onMouseEnter={(e) => (e.currentTarget.style.background = "rgba(229,57,53,0.1)")}
              onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
            >
              删除电影
            </div>
          </div>
        </>
      )}
    </div>
  );
}

// ── Shared styles & components ──

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <p style={{
      margin: "0 0 0.5rem", fontSize: "0.7rem", color: "var(--color-label-quaternary)",
      letterSpacing: "0.08em", textTransform: "uppercase",
    }}>
      {children}
    </p>
  );
}

const smallBtnStyle: React.CSSProperties = {
  background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
  padding: "0.2rem 0.5rem", color: "var(--color-label-tertiary)", cursor: "pointer",
  fontSize: "0.65rem", fontFamily: "inherit",
};

const accentBtnStyle: React.CSSProperties = {
  background: "var(--color-accent)", border: "none", borderRadius: 4,
  padding: "0.2rem 0.6rem", color: "#0B1628", cursor: "pointer",
  fontSize: "0.72rem", fontWeight: 600,
};

const dangerBtnStyle: React.CSSProperties = {
  background: "none", border: "1px solid rgba(229,57,53,0.4)", borderRadius: 4,
  padding: "0.2rem 0.45rem", color: "#e53935", cursor: "pointer",
  fontSize: "0.72rem", fontWeight: 600,
};
