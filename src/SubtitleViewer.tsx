import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SubEntry {
  index: number;
  start_ms: number;
  end_ms: number;
  text: string;
}

function formatTs(ms: number): string {
  const h = Math.floor(ms / 3600000);
  const m = Math.floor((ms % 3600000) / 60000);
  const s = Math.floor((ms % 60000) / 1000);
  const frac = Math.floor((ms % 1000) / 10);
  return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(frac).padStart(2, "0")}`;
}

// ── Minimap ──────────────────────────────────────────────────────
//
// The minimap is a scaled-down rendering of the entire subtitle list.
// It acts as a custom scrollbar: clicking or dragging moves the list.
// The list itself hides its native scrollbar and only scrolls via
// mouse wheel or minimap interaction.

const MINIMAP_WIDTH = 64;

function Minimap({
  entries,
  scrollHeight,
  clientHeight,
  scrollTop,
  onScrollTo,
}: {
  entries: SubEntry[];
  scrollHeight: number;
  clientHeight: number;
  scrollTop: number;
  onScrollTo: (scrollTop: number) => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);
  const dragOffsetRef = useRef(0);

  // Ratio: how much of the total content the minimap represents per pixel
  const containerHeight = containerRef.current?.clientHeight ?? clientHeight;
  const scale = containerHeight > 0 && scrollHeight > 0 ? containerHeight / scrollHeight : 1;

  // Viewport indicator dimensions in minimap space
  const vpH = Math.max(12, clientHeight * scale);
  const vpY = scrollTop * scale;

  // Draw content + viewport
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || entries.length === 0 || scrollHeight <= 0) return;

    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    canvas.width = w * dpr;
    canvas.height = h * dpr;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, w, h);

    // Draw each entry as a thin bar at its proportional position
    const rowH = scrollHeight / entries.length;
    for (const entry of entries) {
      const y = (entry.index - 1) * rowH * scale;
      const lines = entry.text.split("\n").length;
      const barH = Math.max(1, Math.min(3, lines));
      const dur = entry.end_ms - entry.start_ms;
      const alpha = Math.min(0.85, 0.25 + dur / 8000);
      ctx.fillStyle = `rgba(100, 149, 237, ${alpha})`;
      ctx.fillRect(6, y, w - 12, barH);
    }

    // Viewport indicator
    ctx.fillStyle = "rgba(255, 255, 255, 0.1)";
    ctx.fillRect(0, vpY, w, vpH);
    ctx.strokeStyle = "rgba(255, 255, 255, 0.25)";
    ctx.lineWidth = 1;
    ctx.strokeRect(0.5, Math.round(vpY) + 0.5, w - 1, Math.round(vpH) - 1);
  }, [entries, scrollHeight, scale, vpY, vpH]);

  // Convert a minimap Y coordinate to a list scrollTop value
  const yToScrollTop = useCallback(
    (y: number) => {
      if (scale <= 0) return 0;
      // The click point should map to the center of the viewport
      const targetScrollTop = y / scale - clientHeight / 2;
      const maxScroll = scrollHeight - clientHeight;
      return Math.max(0, Math.min(maxScroll, targetScrollTop));
    },
    [scale, clientHeight, scrollHeight],
  );

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const rect = containerRef.current?.getBoundingClientRect();
      if (!rect) return;
      const y = e.clientY - rect.top;

      // Check if clicking on the viewport indicator → start drag with offset
      if (y >= vpY && y <= vpY + vpH) {
        draggingRef.current = true;
        dragOffsetRef.current = y - vpY;
      } else {
        // Click outside viewport → jump (center viewport on click point)
        draggingRef.current = true;
        dragOffsetRef.current = vpH / 2;
        onScrollTo(yToScrollTop(y));
      }

      const handleMouseMove = (me: MouseEvent) => {
        if (!draggingRef.current) return;
        const my = me.clientY - rect.top;
        // Viewport top in minimap space = mouse position - drag offset
        const newVpY = my - dragOffsetRef.current;
        // Convert minimap viewport top to scrollTop
        const newScrollTop = newVpY / scale;
        const maxScroll = scrollHeight - clientHeight;
        onScrollTo(Math.max(0, Math.min(maxScroll, newScrollTop)));
      };

      const handleMouseUp = () => {
        draggingRef.current = false;
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [vpY, vpH, scale, clientHeight, scrollHeight, onScrollTo, yToScrollTop],
  );

  return (
    <div
      ref={containerRef}
      onMouseDown={handleMouseDown}
      style={{
        width: MINIMAP_WIDTH,
        height: "100%",
        cursor: "pointer",
        borderLeft: "1px solid #333",
        flexShrink: 0,
      }}
    >
      <canvas
        ref={canvasRef}
        style={{ width: "100%", height: "100%", display: "block" }}
      />
    </div>
  );
}

// ── Main component ───────────────────────────────────────────────

export function SubtitleViewer() {
  const [entries, setEntries] = useState<SubEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scrollInfo, setScrollInfo] = useState({ scrollTop: 0, scrollHeight: 0, clientHeight: 0 });
  const listRef = useRef<HTMLDivElement>(null);

  const filePath = new URLSearchParams(window.location.search).get("file") ?? "";
  const parts = filePath.replace(/\\/g, "/").split("/");
  const fileName = parts[parts.length - 1] || filePath;

  useEffect(() => {
    if (!filePath) return;
    invoke<SubEntry[]>("parse_subtitle_cmd", { path: filePath })
      .then((data) => {
        setEntries(data);
        setLoading(false);
      })
      .catch((e) => {
        setError(String(e));
        setLoading(false);
      });
  }, [filePath]);

  // Sync scroll state for minimap
  const syncScroll = useCallback(() => {
    const el = listRef.current;
    if (!el) return;
    setScrollInfo({
      scrollTop: el.scrollTop,
      scrollHeight: el.scrollHeight,
      clientHeight: el.clientHeight,
    });
  }, []);

  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    syncScroll();
    el.addEventListener("scroll", syncScroll, { passive: true });
    const ro = new ResizeObserver(syncScroll);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", syncScroll);
      ro.disconnect();
    };
  }, [syncScroll, entries]);

  const handleScrollTo = useCallback((scrollTop: number) => {
    const el = listRef.current;
    if (el) el.scrollTop = scrollTop;
  }, []);

  if (!filePath) {
    return <div style={styles.center}>未指定字幕文件</div>;
  }

  return (
    <div style={styles.container}>
      {/* Title bar */}
      <div style={styles.titleBar} data-tauri-drag-region>
        <span style={styles.fileName}>{fileName}</span>
        <span style={styles.count}>
          {entries.length > 0 ? `${entries.length} 条` : ""}
        </span>
      </div>

      {/* Content area */}
      <div style={styles.body}>
        {loading ? (
          <div style={styles.center}>加载中...</div>
        ) : error ? (
          <div style={{ ...styles.center, color: "#e55" }}>{error}</div>
        ) : entries.length === 0 ? (
          <div style={styles.center}>字幕文件为空</div>
        ) : (
          <>
            {/* Subtitle list — native scrollbar hidden, scroll via wheel + minimap */}
            <div ref={listRef} style={styles.list} className="hide-scrollbar">
              {entries.map((e) => (
                <div key={e.index} style={styles.row}>
                  <div style={styles.timestamp}>
                    {formatTs(e.start_ms)} → {formatTs(e.end_ms)}
                  </div>
                  <div style={styles.text}>{e.text}</div>
                </div>
              ))}
            </div>

            {/* Minimap — custom scrollbar */}
            <Minimap
              entries={entries}
              scrollHeight={scrollInfo.scrollHeight}
              clientHeight={scrollInfo.clientHeight}
              scrollTop={scrollInfo.scrollTop}
              onScrollTo={handleScrollTo}
            />
          </>
        )}
      </div>

      {/* CSS to hide native scrollbar */}
      <style>{`
        .hide-scrollbar::-webkit-scrollbar { display: none; }
        .hide-scrollbar { scrollbar-width: none; -ms-overflow-style: none; }
      `}</style>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100vh",
    background: "#1a1a1a",
    color: "#e0e0e0",
    fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    userSelect: "none",
  },
  titleBar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "10px 16px",
    fontSize: "0.82rem",
    borderBottom: "1px solid #333",
    flexShrink: 0,
  },
  fileName: {
    fontWeight: 600,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  count: {
    color: "#888",
    fontSize: "0.75rem",
    flexShrink: 0,
    marginLeft: 12,
  },
  body: {
    flex: 1,
    display: "flex",
    overflow: "hidden",
  },
  list: {
    flex: 1,
    overflowY: "auto",
    padding: "8px 0",
  },
  row: {
    padding: "6px 16px",
    borderBottom: "1px solid #2a2a2a",
    minHeight: 40,
  },
  timestamp: {
    fontSize: "0.68rem",
    color: "#6495ED",
    marginBottom: 2,
    fontVariantNumeric: "tabular-nums",
  },
  text: {
    fontSize: "0.82rem",
    color: "#ddd",
    lineHeight: 1.4,
    whiteSpace: "pre-wrap",
  },
  center: {
    flex: 1,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    color: "#888",
    fontSize: "0.85rem",
  },
};
