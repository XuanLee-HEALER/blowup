import { useState, useEffect, useCallback } from "react";
import { library } from "../lib/tauri";
import type {
  FilmListEntry,
  FilmFilterResult,
  FilmFilterParams,
  LibraryStats,
  LibraryItemSummary,
  GenreTreeNode,
} from "../lib/tauri";
import { open } from "@tauri-apps/plugin-dialog";

// ── Helpers ──────────────────────────────────────────────────────

function formatSize(bytes: number | null): string {
  if (!bytes) return "—";
  if (bytes >= 1e9) return (bytes / 1e9).toFixed(1) + " GB";
  if (bytes >= 1e6) return (bytes / 1e6).toFixed(0) + " MB";
  return (bytes / 1e3).toFixed(0) + " KB";
}

function formatDuration(secs: number | null): string {
  if (!secs) return "—";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h > 0 ? `${h}h${m}m` : `${m}m`;
}

function flattenGenres(
  nodes: GenreTreeNode[]
): { id: number; name: string }[] {
  const result: { id: number; name: string }[] = [];
  function walk(ns: GenreTreeNode[]) {
    for (const n of ns) {
      result.push({ id: n.id, name: n.name });
      walk(n.children);
    }
  }
  walk(nodes);
  return result;
}

// ── Stats Bar ────────────────────────────────────────────────────

function StatsBar({
  stats,
  onScan,
}: {
  stats: LibraryStats | null;
  onScan: () => void;
}) {
  if (!stats) return null;
  const sizeStr = formatSize(stats.total_file_size);
  const pct =
    stats.total_films > 0
      ? ((stats.films_with_files / stats.total_films) * 100).toFixed(0)
      : "0";
  return (
    <div
      style={{
        padding: "10px 16px",
        borderBottom: "1px solid var(--color-separator)",
        display: "flex",
        alignItems: "center",
        gap: 16,
        fontSize: 13,
        color: "var(--color-label-secondary)",
      }}
    >
      <span>{stats.total_films} 部影片</span>
      <span>{stats.films_with_files} 部已关联</span>
      <span>{sizeStr}</span>
      <span>{pct}%</span>
      {stats.unlinked_files > 0 && (
        <span style={{ color: "var(--color-accent)" }}>
          {stats.unlinked_files} 个待关联
        </span>
      )}
      <button
        onClick={onScan}
        style={{
          marginLeft: "auto",
          background: "var(--color-bg-control)",
          border: "1px solid var(--color-separator)",
          borderRadius: 6,
          padding: "4px 12px",
          color: "var(--color-label-primary)",
          cursor: "pointer",
          fontSize: 13,
        }}
      >
        扫描目录
      </button>
    </div>
  );
}

// ── Film Card ────────────────────────────────────────────────────

function FilmCard({
  film,
  selected,
  onClick,
}: {
  film: FilmListEntry;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <div
      onClick={onClick}
      style={{
        padding: "10px 16px",
        cursor: "pointer",
        borderBottom: "1px solid var(--color-separator)",
        background: selected ? "var(--color-bg-selected)" : "transparent",
        display: "flex",
        alignItems: "center",
        gap: 10,
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            fontWeight: 500,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {film.title}
        </div>
        <div style={{ fontSize: 12, color: "var(--color-label-secondary)" }}>
          {film.year ?? "—"}{" "}
          {film.tmdb_rating ? `★ ${film.tmdb_rating.toFixed(1)}` : ""}
        </div>
      </div>
      <span
        style={{
          fontSize: 14,
          color: film.has_file
            ? "var(--color-accent)"
            : "var(--color-label-tertiary)",
        }}
      >
        {film.has_file ? "✓" : "✗"}
      </span>
    </div>
  );
}

// ── File Card ────────────────────────────────────────────────────

function FileCard({
  item,
  selected,
  onClick,
}: {
  item: LibraryItemSummary;
  selected: boolean;
  onClick: () => void;
}) {
  const fileName = item.file_path.split(/[/\\]/).pop() ?? item.file_path;
  return (
    <div
      onClick={onClick}
      style={{
        padding: "10px 16px",
        cursor: "pointer",
        borderBottom: "1px solid var(--color-separator)",
        background: selected ? "var(--color-bg-selected)" : "transparent",
      }}
    >
      <div
        style={{
          fontWeight: 500,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
          fontSize: 13,
        }}
      >
        {fileName}
      </div>
      <div style={{ fontSize: 12, color: "var(--color-label-secondary)" }}>
        {formatSize(item.file_size)} · {item.resolution ?? "—"} ·{" "}
        {item.video_codec ?? "—"}
      </div>
    </div>
  );
}

// ── Film Detail Panel ────────────────────────────────────────────

function FilmDetailView({
  film,
  items,
  onLink,
  onUnlink,
  onRemoveItem,
}: {
  film: FilmListEntry;
  items: LibraryItemSummary[];
  onLink: () => void;
  onUnlink: (itemId: number) => void;
  onRemoveItem: (itemId: number) => void;
}) {
  const linkedItems = items.filter((i) => i.film_id === film.id);
  return (
    <div>
      <h2 style={{ margin: "0 0 4px" }}>{film.title}</h2>
      {film.original_title && (
        <div
          style={{
            color: "var(--color-label-secondary)",
            fontSize: 14,
            marginBottom: 4,
          }}
        >
          {film.original_title}
        </div>
      )}
      <div style={{ color: "var(--color-label-secondary)", fontSize: 13, marginBottom: 16 }}>
        {film.year ?? "—"} · {film.tmdb_rating ? `★ ${film.tmdb_rating.toFixed(1)}` : "未评分"}
      </div>

      <h3 style={{ fontSize: 14, marginBottom: 8 }}>本地文件</h3>
      {linkedItems.length === 0 ? (
        <div style={{ color: "var(--color-label-tertiary)", fontSize: 13, marginBottom: 8 }}>
          暂无关联文件
        </div>
      ) : (
        linkedItems.map((item) => (
          <div
            key={item.id}
            style={{
              background: "var(--color-bg-control)",
              borderRadius: 8,
              padding: 12,
              marginBottom: 8,
              fontSize: 13,
            }}
          >
            <div style={{ wordBreak: "break-all", marginBottom: 6 }}>
              {item.file_path.split(/[/\\]/).pop()}
            </div>
            <div style={{ color: "var(--color-label-secondary)", display: "flex", gap: 12, flexWrap: "wrap" }}>
              <span>{formatSize(item.file_size)}</span>
              <span>{item.resolution ?? "—"}</span>
              <span>{item.video_codec ?? "—"}/{item.audio_codec ?? "—"}</span>
              <span>{formatDuration(item.duration_secs)}</span>
            </div>
            <div style={{ marginTop: 8, display: "flex", gap: 8 }}>
              <button
                onClick={() => onUnlink(item.id)}
                style={{
                  background: "none",
                  border: "1px solid var(--color-separator)",
                  borderRadius: 4,
                  padding: "2px 8px",
                  color: "var(--color-label-secondary)",
                  cursor: "pointer",
                  fontSize: 12,
                }}
              >
                取消关联
              </button>
              <button
                onClick={() => onRemoveItem(item.id)}
                style={{
                  background: "none",
                  border: "1px solid #e53935",
                  borderRadius: 4,
                  padding: "2px 8px",
                  color: "#e53935",
                  cursor: "pointer",
                  fontSize: 12,
                }}
              >
                移除
              </button>
            </div>
          </div>
        ))
      )}
      <button
        onClick={onLink}
        style={{
          background: "var(--color-accent)",
          color: "#fff",
          border: "none",
          borderRadius: 6,
          padding: "6px 16px",
          cursor: "pointer",
          fontSize: 13,
        }}
      >
        关联文件
      </button>
    </div>
  );
}

// ── File Detail Panel ────────────────────────────────────────────

function FileDetailView({
  item,
  films,
  onLink,
  onRemove,
}: {
  item: LibraryItemSummary;
  films: FilmListEntry[];
  onLink: (filmId: number) => void;
  onRemove: () => void;
}) {
  const [searchQ, setSearchQ] = useState("");
  const filtered = searchQ.length >= 2
    ? films.filter((f) =>
        f.title.toLowerCase().includes(searchQ.toLowerCase())
      )
    : [];

  return (
    <div>
      <h2 style={{ margin: "0 0 8px", fontSize: 16, wordBreak: "break-all" }}>
        {item.file_path.split(/[/\\]/).pop()}
      </h2>
      <div style={{ fontSize: 13, color: "var(--color-label-secondary)", marginBottom: 12, wordBreak: "break-all" }}>
        {item.file_path}
      </div>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: 8,
          fontSize: 13,
          marginBottom: 16,
        }}
      >
        <div>大小: {formatSize(item.file_size)}</div>
        <div>时长: {formatDuration(item.duration_secs)}</div>
        <div>视频: {item.video_codec ?? "—"}</div>
        <div>音频: {item.audio_codec ?? "—"}</div>
        <div>分辨率: {item.resolution ?? "—"}</div>
      </div>

      {item.film_id ? (
        <div style={{ fontSize: 13, color: "var(--color-label-secondary)" }}>
          已关联: {item.film_title} ({item.film_year})
        </div>
      ) : (
        <div>
          <h3 style={{ fontSize: 14, marginBottom: 8 }}>关联到影片</h3>
          <input
            placeholder="搜索影片名称..."
            value={searchQ}
            onChange={(e) => setSearchQ(e.target.value)}
            style={{
              width: "100%",
              padding: "6px 10px",
              borderRadius: 6,
              border: "1px solid var(--color-separator)",
              background: "var(--color-bg-control)",
              color: "var(--color-label-primary)",
              fontSize: 13,
              marginBottom: 8,
              boxSizing: "border-box",
            }}
          />
          {filtered.slice(0, 10).map((f) => (
            <div
              key={f.id}
              onClick={() => onLink(f.id)}
              style={{
                padding: "6px 10px",
                cursor: "pointer",
                borderBottom: "1px solid var(--color-separator)",
                fontSize: 13,
              }}
            >
              {f.title} ({f.year ?? "—"})
            </div>
          ))}
        </div>
      )}

      <button
        onClick={onRemove}
        style={{
          marginTop: 16,
          background: "none",
          border: "1px solid #e53935",
          borderRadius: 6,
          padding: "6px 16px",
          color: "#e53935",
          cursor: "pointer",
          fontSize: 13,
        }}
      >
        移除文件
      </button>
    </div>
  );
}

// ── Main Page ────────────────────────────────────────────────────

export default function Library() {
  const [tab, setTab] = useState<"films" | "files">("films");
  const [stats, setStats] = useState<LibraryStats | null>(null);

  // Films tab state
  const [filmResult, setFilmResult] = useState<FilmFilterResult | null>(null);
  const [filters, setFilters] = useState<FilmFilterParams>({
    sortBy: "added",
    sortDesc: true,
    page: 1,
    pageSize: 50,
  });
  const [selectedFilmId, setSelectedFilmId] = useState<number | null>(null);
  const [genres, setGenres] = useState<{ id: number; name: string }[]>([]);

  // Files tab state
  const [items, setItems] = useState<LibraryItemSummary[]>([]);
  const [selectedItemId, setSelectedItemId] = useState<number | null>(null);

  // All films for file-linking search
  const [allFilms, setAllFilms] = useState<FilmListEntry[]>([]);

  const refresh = useCallback(() => {
    library.getLibraryStats().then(setStats);
    library.listLibraryItems().then(setItems);
    library
      .listFilmsFiltered({ pageSize: 9999 })
      .then((r) => setAllFilms(r.films));
  }, []);

  useEffect(() => {
    refresh();
    library
      .listGenresTree()
      .then((tree) => setGenres(flattenGenres(tree)));
  }, [refresh]);

  // Reload films when filters change
  useEffect(() => {
    library.listFilmsFiltered(filters).then(setFilmResult);
  }, [filters]);

  const handleScan = async () => {
    const dir = await open({ directory: true });
    if (!dir) return;
    const result = await library.scanLibraryDirectory(dir as string);
    alert(
      `扫描完成: 添加 ${result.added} 个, 跳过 ${result.skipped} 个` +
        (result.errors.length > 0
          ? `, 错误 ${result.errors.length} 个`
          : "")
    );
    refresh();
  };

  const handleLinkFile = async (filmId: number) => {
    const filePath = await open({
      multiple: false,
      filters: [
        {
          name: "Video",
          extensions: [
            "mp4", "mkv", "avi", "mov", "ts", "webm", "m4v", "flv", "wmv",
          ],
        },
      ],
    });
    if (!filePath) return;
    await library.addLibraryItem(filePath as string, filmId);
    refresh();
  };

  const handleUnlink = async (itemId: number) => {
    await library.unlinkItemFromFilm(itemId);
    refresh();
  };

  const handleRemoveItem = async (itemId: number) => {
    await library.removeLibraryItem(itemId);
    refresh();
  };

  const handleLinkItemToFilm = async (
    itemId: number,
    filmId: number
  ) => {
    await library.linkItemToFilm(itemId, filmId);
    refresh();
  };

  const selectedFilm = filmResult?.films.find(
    (f) => f.id === selectedFilmId
  );
  const selectedItem = items.find((i) => i.id === selectedItemId);
  const unlinkedItems = items.filter((i) => !i.film_id);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <StatsBar stats={stats} onScan={handleScan} />

      {/* Tab bar */}
      <div
        style={{
          display: "flex",
          borderBottom: "1px solid var(--color-separator)",
        }}
      >
        {(
          [
            ["films", "影片"],
            ["files", `待关联文件 (${unlinkedItems.length})`],
          ] as const
        ).map(([key, label]) => (
          <button
            key={key}
            onClick={() => setTab(key)}
            style={{
              flex: 1,
              padding: "8px 0",
              background: "none",
              border: "none",
              borderBottom:
                tab === key ? "2px solid var(--color-accent)" : "2px solid transparent",
              color:
                tab === key
                  ? "var(--color-accent)"
                  : "var(--color-label-secondary)",
              cursor: "pointer",
              fontWeight: tab === key ? 600 : 400,
              fontSize: 13,
            }}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Content area */}
      <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>
        {/* Left panel */}
        <div
          style={{
            width: 340,
            borderRight: "1px solid var(--color-separator)",
            display: "flex",
            flexDirection: "column",
            overflow: "hidden",
          }}
        >
          {tab === "films" && (
            <>
              {/* Filter bar */}
              <div style={{ padding: "8px 12px", borderBottom: "1px solid var(--color-separator)" }}>
                <input
                  placeholder="搜索影片..."
                  value={filters.query ?? ""}
                  onChange={(e) =>
                    setFilters((prev) => ({
                      ...prev,
                      query: e.target.value || undefined,
                      page: 1,
                    }))
                  }
                  style={{
                    width: "100%",
                    padding: "6px 10px",
                    borderRadius: 6,
                    border: "1px solid var(--color-separator)",
                    background: "var(--color-bg-control)",
                    color: "var(--color-label-primary)",
                    fontSize: 13,
                    boxSizing: "border-box",
                    marginBottom: 6,
                  }}
                />
                <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                  <select
                    value={filters.genreId ?? ""}
                    onChange={(e) =>
                      setFilters((prev) => ({
                        ...prev,
                        genreId: e.target.value
                          ? Number(e.target.value)
                          : undefined,
                        page: 1,
                      }))
                    }
                    style={{
                      flex: 1,
                      minWidth: 80,
                      background: "var(--color-bg-control)",
                      border: "1px solid var(--color-separator)",
                      borderRadius: 4,
                      padding: "3px 6px",
                      color: "var(--color-label-primary)",
                      fontSize: 12,
                    }}
                  >
                    <option value="">全部类型</option>
                    {genres.map((g) => (
                      <option key={g.id} value={g.id}>
                        {g.name}
                      </option>
                    ))}
                  </select>
                  <select
                    value={
                      filters.hasFile === true
                        ? "yes"
                        : filters.hasFile === false
                        ? "no"
                        : ""
                    }
                    onChange={(e) =>
                      setFilters((prev) => ({
                        ...prev,
                        hasFile:
                          e.target.value === "yes"
                            ? true
                            : e.target.value === "no"
                            ? false
                            : undefined,
                        page: 1,
                      }))
                    }
                    style={{
                      flex: 1,
                      minWidth: 80,
                      background: "var(--color-bg-control)",
                      border: "1px solid var(--color-separator)",
                      borderRadius: 4,
                      padding: "3px 6px",
                      color: "var(--color-label-primary)",
                      fontSize: 12,
                    }}
                  >
                    <option value="">全部状态</option>
                    <option value="yes">已关联</option>
                    <option value="no">未关联</option>
                  </select>
                  <select
                    value={filters.sortBy ?? "added"}
                    onChange={(e) =>
                      setFilters((prev) => ({
                        ...prev,
                        sortBy: e.target.value,
                        page: 1,
                      }))
                    }
                    style={{
                      flex: 1,
                      minWidth: 80,
                      background: "var(--color-bg-control)",
                      border: "1px solid var(--color-separator)",
                      borderRadius: 4,
                      padding: "3px 6px",
                      color: "var(--color-label-primary)",
                      fontSize: 12,
                    }}
                  >
                    <option value="added">最近添加</option>
                    <option value="title">标题</option>
                    <option value="year">年份</option>
                    <option value="rating">评分</option>
                  </select>
                </div>
              </div>
              {/* Film list */}
              <div style={{ flex: 1, overflowY: "auto" }}>
                {filmResult?.films.map((film) => (
                  <FilmCard
                    key={film.id}
                    film={film}
                    selected={film.id === selectedFilmId}
                    onClick={() => setSelectedFilmId(film.id)}
                  />
                ))}
                {filmResult && filmResult.total > filmResult.films.length && (
                  <div style={{ padding: 12, textAlign: "center" }}>
                    <button
                      onClick={() =>
                        setFilters((prev) => ({
                          ...prev,
                          pageSize: (prev.pageSize ?? 50) + 50,
                        }))
                      }
                      style={{
                        background: "none",
                        border: "1px solid var(--color-separator)",
                        borderRadius: 6,
                        padding: "6px 20px",
                        color: "var(--color-label-secondary)",
                        cursor: "pointer",
                        fontSize: 13,
                      }}
                    >
                      加载更多
                    </button>
                  </div>
                )}
              </div>
            </>
          )}

          {tab === "files" && (
            <div style={{ flex: 1, overflowY: "auto" }}>
              {unlinkedItems.length === 0 ? (
                <div
                  style={{
                    padding: 24,
                    textAlign: "center",
                    color: "var(--color-label-tertiary)",
                    fontSize: 13,
                  }}
                >
                  没有待关联文件。点击"扫描目录"添加文件。
                </div>
              ) : (
                unlinkedItems.map((item) => (
                  <FileCard
                    key={item.id}
                    item={item}
                    selected={item.id === selectedItemId}
                    onClick={() => setSelectedItemId(item.id)}
                  />
                ))
              )}
            </div>
          )}
        </div>

        {/* Right panel */}
        <div style={{ flex: 1, overflowY: "auto", padding: 24 }}>
          {tab === "films" && selectedFilm ? (
            <FilmDetailView
              film={selectedFilm}
              items={items}
              onLink={() => handleLinkFile(selectedFilm.id)}
              onUnlink={handleUnlink}
              onRemoveItem={handleRemoveItem}
            />
          ) : tab === "files" && selectedItem ? (
            <FileDetailView
              item={selectedItem}
              films={allFilms}
              onLink={(filmId) =>
                handleLinkItemToFilm(selectedItem.id, filmId)
              }
              onRemove={() => handleRemoveItem(selectedItem.id)}
            />
          ) : (
            <div
              style={{
                height: "100%",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                color: "var(--color-label-tertiary)",
              }}
            >
              {tab === "films"
                ? "选择一部影片查看详情"
                : "选择一个文件查看详情"}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
