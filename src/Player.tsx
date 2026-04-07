import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface PlayerState {
  playing: boolean;
  position: number;
  duration: number;
  volume: number;
  paused: boolean;
  title: string;
}

interface TrackInfo {
  id: number;
  track_type: string;
  title: string | null;
  lang: string | null;
  selected: boolean;
}

function formatTime(s: number): string {
  if (!s || !isFinite(s)) return "0:00";
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = Math.floor(s % 60);
  return h > 0
    ? `${h}:${m.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`
    : `${m}:${sec.toString().padStart(2, "0")}`;
}

function trackLabel(t: TrackInfo): string {
  const parts: string[] = [];
  if (t.title) parts.push(t.title);
  if (t.lang) parts.push(t.lang.toUpperCase());
  return parts.length > 0 ? parts.join(" — ") : `Track ${t.id}`;
}

// ── Icons (SVG inline, Apple SF-style) ──────────────────────────

const PlayIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
    <path d="M8 5.14v14.72a1 1 0 001.5.86l11.5-7.36a1 1 0 000-1.72L9.5 4.28A1 1 0 008 5.14z" />
  </svg>
);
const PauseIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
    <rect x="6" y="4" width="4" height="16" rx="1" />
    <rect x="14" y="4" width="4" height="16" rx="1" />
  </svg>
);
const VolumeIcon = ({ muted }: { muted: boolean }) => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
    <path d="M11 5L6 9H3a1 1 0 00-1 1v4a1 1 0 001 1h3l5 4V5z" />
    {!muted && (
      <>
        <path d="M15.54 8.46a5 5 0 010 7.08" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        <path d="M18.07 5.93a9 9 0 010 12.14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </>
    )}
    {muted && <line x1="18" y1="9" x2="14" y2="15" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />}
  </svg>
);
const FullscreenIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
    <path d="M8 3H5a2 2 0 00-2 2v3m18 0V5a2 2 0 00-2-2h-3m0 18h3a2 2 0 002-2v-3M3 16v3a2 2 0 002 2h3" />
  </svg>
);

// ── Main Player Component ───────────────────────────────────────

export function Player() {
  const [state, setState] = useState<PlayerState>({
    playing: true, position: 0, duration: 0, volume: 100, paused: false, title: "",
  });
  const [showControls, setShowControls] = useState(true);
  const [seeking, setSeeking] = useState(false);
  const [seekPos, setSeekPos] = useState(0);
  const [showTracks, setShowTracks] = useState(false);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const hideTimer = useRef<number | null>(null);

  // Listen to player-state events
  useEffect(() => {
    const unlisten = listen<PlayerState>("player-state", (event) => {
      if (!seeking) setState(event.payload);
    });
    return () => { unlisten.then((f) => f()); };
  }, [seeking]);

  // Auto-hide controls
  const resetHideTimer = useCallback(() => {
    setShowControls(true);
    if (hideTimer.current) clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      if (!seeking && !showTracks) setShowControls(false);
    }, 3000);
  }, [seeking, showTracks]);

  useEffect(() => {
    const onMove = () => resetHideTimer();
    window.addEventListener("mousemove", onMove);
    return () => {
      window.removeEventListener("mousemove", onMove);
      if (hideTimer.current) clearTimeout(hideTimer.current);
    };
  }, [resetHideTimer]);

  // Commands
  const playPause = () => invoke("cmd_player_play_pause");
  const seek = (pos: number) => invoke("cmd_player_seek", { position: pos });
  const setVolume = (v: number) => invoke("cmd_player_set_volume", { volume: v });
  const toggleFullscreen = () => invoke("cmd_player_toggle_fullscreen");
  const setSubTrack = (id: number) => invoke("cmd_player_set_subtitle_track", { trackId: id });
  const setAudioTrack = (id: number) => invoke("cmd_player_set_audio_track", { trackId: id });

  const loadTracks = async () => {
    const t = await invoke<TrackInfo[]>("cmd_player_get_tracks");
    setTracks(t);
    setShowTracks(true);
  };

  // Keyboard shortcuts
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.isComposing) return;
      switch (e.key) {
        case " ": e.preventDefault(); playPause(); break;
        case "ArrowLeft": invoke("cmd_player_seek_relative", { offset: -5 }); break;
        case "ArrowRight": invoke("cmd_player_seek_relative", { offset: 5 }); break;
        case "ArrowUp": e.preventDefault(); setVolume(Math.min(100, state.volume + 5)); break;
        case "ArrowDown": e.preventDefault(); setVolume(Math.max(0, state.volume - 5)); break;
        case "f": toggleFullscreen(); break;
        case "Escape": if (showTracks) { setShowTracks(false); } break;
        case "m": setVolume(state.volume > 0 ? 0 : 100); break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [state.volume, showTracks]);

  const progress = state.duration > 0 ? ((seeking ? seekPos : state.position) / state.duration) * 100 : 0;

  return (
    <div
      style={{
        position: "fixed", inset: 0,
        background: "rgba(0,0,0,0.01)",
        display: "flex", flexDirection: "column",
        cursor: showControls ? "default" : "none",
        fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif",
        userSelect: "none",
      }}
      onMouseMove={resetHideTimer}
    >
      {/* Click area */}
      <div style={{ flex: 1 }} onClick={() => playPause()} onDoubleClick={() => toggleFullscreen()} />

      {/* Track selector panel */}
      {showTracks && (
        <div style={{
          position: "absolute", right: 16, bottom: 100,
          background: "rgba(30,30,30,0.95)", backdropFilter: "blur(20px)",
          borderRadius: 10, padding: "8px 0", minWidth: 200,
          boxShadow: "0 8px 32px rgba(0,0,0,0.5)",
        }}>
          {["audio", "sub"].map((type) => {
            const items = tracks.filter((t) => t.track_type === type);
            if (items.length === 0) return null;
            return (
              <div key={type}>
                <div style={{ padding: "6px 14px", fontSize: 11, color: "rgba(255,255,255,0.4)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
                  {type === "audio" ? "音轨" : "字幕"}
                </div>
                {type === "sub" && (
                  <div
                    onClick={() => { setSubTrack(0); setShowTracks(false); }}
                    style={{ ...trackItemStyle, color: tracks.every((t) => t.track_type === "sub" && !t.selected) ? "#fff" : "rgba(255,255,255,0.6)" }}
                  >
                    关闭
                  </div>
                )}
                {items.map((t) => (
                  <div
                    key={t.id}
                    onClick={() => { (type === "audio" ? setAudioTrack : setSubTrack)(t.id); setShowTracks(false); }}
                    style={{ ...trackItemStyle, color: t.selected ? "#fff" : "rgba(255,255,255,0.6)" }}
                  >
                    {t.selected && <span style={{ marginRight: 6 }}>✓</span>}
                    {trackLabel(t)}
                  </div>
                ))}
              </div>
            );
          })}
        </div>
      )}

      {/* Controls bar — Apple style */}
      <div
        style={{
          background: "linear-gradient(transparent, rgba(0,0,0,0.7))",
          padding: "40px 20px 16px",
          opacity: showControls ? 1 : 0,
          transition: "opacity 0.3s ease",
          pointerEvents: showControls ? "auto" : "none",
        }}
      >
        {/* Progress bar */}
        <div
          style={{
            height: 3, background: "rgba(255,255,255,0.2)", borderRadius: 1.5,
            cursor: "pointer", marginBottom: 12, position: "relative",
            transition: "height 0.15s ease",
          }}
          onMouseEnter={(e) => { (e.currentTarget as HTMLElement).style.height = "6px"; }}
          onMouseLeave={(e) => { if (!seeking) (e.currentTarget as HTMLElement).style.height = "3px"; }}
          onMouseDown={(e) => {
            const bar = e.currentTarget;
            const rect = bar.getBoundingClientRect();
            const update = (clientX: number) => {
              const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
              setSeekPos(ratio * state.duration);
            };
            setSeeking(true);
            update(e.clientX);
            const onMove = (ev: MouseEvent) => update(ev.clientX);
            const onUp = (ev: MouseEvent) => {
              const ratio = Math.max(0, Math.min(1, (ev.clientX - rect.left) / rect.width));
              seek(ratio * state.duration);
              setSeeking(false);
              bar.style.height = "3px";
              window.removeEventListener("mousemove", onMove);
              window.removeEventListener("mouseup", onUp);
            };
            window.addEventListener("mousemove", onMove);
            window.addEventListener("mouseup", onUp);
          }}
        >
          <div style={{
            height: "100%", borderRadius: 1.5, background: "rgba(255,255,255,0.9)",
            width: `${progress}%`, transition: seeking ? "none" : "width 0.1s linear",
          }} />
        </div>

        {/* Button row */}
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          {/* Play/Pause */}
          <button onClick={() => playPause()} style={iconBtn}>
            {state.paused ? <PlayIcon /> : <PauseIcon />}
          </button>

          {/* Volume */}
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <button onClick={() => setVolume(state.volume > 0 ? 0 : 100)} style={iconBtn}>
              <VolumeIcon muted={state.volume === 0} />
            </button>
            <input
              type="range" min="0" max="100" value={state.volume}
              onChange={(e) => setVolume(Number(e.target.value))}
              style={{ width: 70, accentColor: "white", height: 3 }}
            />
          </div>

          {/* Time */}
          <span style={{ fontSize: 12, color: "rgba(255,255,255,0.7)", fontVariantNumeric: "tabular-nums" }}>
            {formatTime(seeking ? seekPos : state.position)}
            <span style={{ margin: "0 3px", opacity: 0.4 }}>/</span>
            {formatTime(state.duration)}
          </span>

          <div style={{ flex: 1 }} />

          {/* Title */}
          <span style={{
            fontSize: 12, color: "rgba(255,255,255,0.5)",
            maxWidth: 300, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
          }}>
            {state.title}
          </span>

          {/* Tracks */}
          <button onClick={() => showTracks ? setShowTracks(false) : loadTracks()} style={iconBtn} title="字幕/音轨">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <rect x="2" y="3" width="20" height="18" rx="2" />
              <path d="M7 15h4m-4-3h10m-10-3h6" />
            </svg>
          </button>

          {/* Fullscreen */}
          <button onClick={() => toggleFullscreen()} style={iconBtn} title="全屏">
            <FullscreenIcon />
          </button>
        </div>
      </div>
    </div>
  );
}

const iconBtn: React.CSSProperties = {
  background: "none", border: "none", color: "rgba(255,255,255,0.85)",
  cursor: "pointer", padding: 4, borderRadius: 4, lineHeight: 0,
  display: "flex", alignItems: "center", justifyContent: "center",
};

const trackItemStyle: React.CSSProperties = {
  padding: "7px 14px", fontSize: 13, cursor: "pointer",
};
