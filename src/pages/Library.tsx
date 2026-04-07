import { useState, useEffect, useCallback } from "react";
import { library, media } from "../lib/tauri";
import type { IndexEntry } from "../lib/tauri";
import { TextInput } from "../components/ui/TextInput";

export default function Library() {
  const [directorMap, setDirectorMap] = useState<Record<string, IndexEntry[]>>({});
  const [selectedDirector, setSelectedDirector] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<IndexEntry | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<IndexEntry[] | null>(null);

  const refresh = useCallback(async () => {
    const map = await library.listIndexByDirector();
    setDirectorMap(map);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const doSearch = async () => {
    if (!searchQuery.trim()) {
      setSearchResults(null);
      return;
    }
    const results = await library.searchIndex(searchQuery.trim());
    setSearchResults(results);
  };

  const handlePlay = async (entry: IndexEntry, file: string) => {
    const cfg = await import("../lib/tauri").then((m) => m.config.get());
    const root = cfg.library.root_dir;
    const fullPath = `${root}/${entry.path}/${file}`;
    await media.openInPlayer(fullPath);
  };

  const handleRebuild = async () => {
    await library.rebuildIndex();
    await refresh();
  };

  // Directors sorted by name
  const directors = Object.keys(directorMap).sort();
  const directorEntries = selectedDirector ? (directorMap[selectedDirector] ?? []) : [];

  // Display entries: search results or director entries
  const displayEntries = searchResults ?? directorEntries;

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      {/* Left: director list + search */}
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
            <button onClick={handleRebuild} style={{
              background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
              padding: "0.2rem 0.5rem", color: "var(--color-label-tertiary)", cursor: "pointer",
              fontSize: "0.65rem", fontFamily: "inherit",
            }}>重建索引</button>
            {searchResults && (
              <button onClick={() => { setSearchResults(null); setSearchQuery(""); }} style={{
                background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
                padding: "0.2rem 0.5rem", color: "var(--color-label-tertiary)", cursor: "pointer",
                fontSize: "0.65rem", fontFamily: "inherit",
              }}>清除搜索</button>
            )}
          </div>
        </div>

        <div style={{ height: 1, background: "var(--color-separator)", margin: "0 1rem" }} />

        {/* Director list */}
        <div style={{ flex: 1, overflowY: "auto", padding: "0.5rem 0" }}>
          {searchResults ? (
            // Search results mode
            displayEntries.map((e) => (
              <div
                key={e.tmdb_id}
                onClick={() => setSelectedEntry(e)}
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
            // Director tree mode
            directors.map((dir) => (
              <div key={dir}>
                <div
                  onClick={() => {
                    setSelectedDirector(selectedDirector === dir ? null : dir);
                    setSelectedEntry(null);
                  }}
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
                    onClick={() => setSelectedEntry(e)}
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

      {/* Right: detail panel */}
      <div style={{ flex: 1, overflowY: "auto", padding: "1.4rem 1.75rem" }}>
        {selectedEntry ? (
          <div>
            <h2 style={{ margin: "0 0 0.3rem", fontSize: "1.2rem", fontWeight: 700, letterSpacing: "-0.02em" }}>
              {selectedEntry.title}
            </h2>
            <div style={{ fontSize: "0.78rem", color: "var(--color-label-secondary)", marginBottom: "1rem" }}>
              {selectedEntry.director_display}
              {selectedEntry.year && <span> · {selectedEntry.year}</span>}
              {selectedEntry.genres.length > 0 && (
                <span> · {selectedEntry.genres.join(", ")}</span>
              )}
            </div>

            <p style={{
              margin: "0 0 0.5rem", fontSize: "0.7rem", color: "var(--color-label-quaternary)",
              letterSpacing: "0.08em", textTransform: "uppercase",
            }}>文件</p>

            {selectedEntry.files.length === 0 ? (
              <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>无文件</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}>
                {selectedEntry.files.map((file) => (
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
                    {(file.endsWith(".mkv") || file.endsWith(".mp4") || file.endsWith(".avi")) && (
                      <button
                        onClick={() => handlePlay(selectedEntry, file)}
                        style={{
                          background: "var(--color-accent)", border: "none", borderRadius: 4,
                          padding: "0.2rem 0.6rem", color: "#0B1628", cursor: "pointer",
                          fontSize: "0.72rem", fontWeight: 600, flexShrink: 0, marginLeft: "0.5rem",
                        }}
                      >
                        ▶ 播放
                      </button>
                    )}
                  </div>
                ))}
              </div>
            )}

            <div style={{ marginTop: "1rem", fontSize: "0.7rem", color: "var(--color-label-quaternary)" }}>
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
    </div>
  );
}
