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

// ── Glass design tokens ─────────────────────────────────────────

const glass = {
  bg: "rgba(255, 255, 255, 0.06)",
  bgHover: "rgba(255, 255, 255, 0.12)",
  bgActive: "rgba(255, 255, 255, 0.18)",
  border: "1px solid rgba(255, 255, 255, 0.12)",
  borderLight: "1px solid rgba(255, 255, 255, 0.08)",
  backdrop: "blur(40px) saturate(180%)",
  shadow: "0 8px 32px rgba(0, 0, 0, 0.35), inset 0 1px 0 rgba(255, 255, 255, 0.08)",
  shadowSmall: "0 2px 8px rgba(0, 0, 0, 0.3)",
  radius: 14,
  radiusSmall: 8,
  text: "rgba(255, 255, 255, 0.9)",
  textDim: "rgba(255, 255, 255, 0.5)",
  trackBg: "rgba(255, 255, 255, 0.15)",
  trackFill: "rgba(255, 255, 255, 0.85)",
};

// ── Icons (SVG inline, Apple SF-style) ──────────────────────────

const PlayIcon = () => (
  <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
    <path d="M8 5.14v14.72a1 1 0 001.5.86l11.5-7.36a1 1 0 000-1.72L9.5 4.28A1 1 0 008 5.14z" />
  </svg>
);
const PauseIcon = () => (
  <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
    <rect x="6" y="4" width="4" height="16" rx="1" />
    <rect x="14" y="4" width="4" height="16" rx="1" />
  </svg>
);
const VolumeIcon = ({ level }: { level: number }) => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
    <path d="M11 5L6 9H3a1 1 0 00-1 1v4a1 1 0 001 1h3l5 4V5z" />
    {level > 0 && (
      <path d="M15.54 8.46a5 5 0 010 7.08" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    )}
    {level > 50 && (
      <path d="M18.07 5.93a9 9 0 010 12.14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    )}
    {level === 0 && <line x1="18" y1="9" x2="14" y2="15" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />}
  </svg>
);
const FullscreenIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
    <path d="M8 3H5a2 2 0 00-2 2v3m18 0V5a2 2 0 00-2-2h-3m0 18h3a2 2 0 002-2v-3M3 16v3a2 2 0 002 2h3" />
  </svg>
);

// ── Custom volume slider ────────────────────────────────────────

function VolumeSlider({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  const barRef = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);

  const handleMouse = useCallback((clientX: number) => {
    if (!barRef.current) return;
    const rect = barRef.current.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    onChange(Math.round(ratio * 100));
  }, [onChange]);

  return (
    <div
      ref={barRef}
      style={{
        width: 72, height: 4, borderRadius: 2,
        background: glass.trackBg,
        cursor: "pointer", position: "relative",
      }}
      onMouseDown={(e) => {
        handleMouse(e.clientX);
        setDragging(true);
        const onMove = (ev: MouseEvent) => handleMouse(ev.clientX);
        const onUp = () => {
          setDragging(false);
          window.removeEventListener("mousemove", onMove);
          window.removeEventListener("mouseup", onUp);
        };
        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onUp);
      }}
    >
      {/* Fill */}
      <div style={{
        position: "absolute", inset: 0,
        borderRadius: 2,
        background: glass.trackFill,
        width: `${value}%`,
        transition: dragging ? "none" : "width 0.1s ease",
      }} />
      {/* Knob */}
      <div style={{
        position: "absolute",
        top: "50%", left: `${value}%`,
        transform: "translate(-50%, -50%)",
        width: 10, height: 10, borderRadius: "50%",
        background: "#fff",
        boxShadow: glass.shadowSmall,
        opacity: dragging ? 1 : 0,
        transition: "opacity 0.15s ease",
        pointerEvents: "none",
      }} />
    </div>
  );
}

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
  const [hoverTime, setHoverTime] = useState<{ x: number; time: number } | null>(null);
  const hideTimer = useRef<number | null>(null);
  const seekTarget = useRef<number | null>(null);
  const progressRef = useRef<HTMLDivElement>(null);

  // Listen to player-state events (push model: mpv pushes property changes)
  useEffect(() => {
    const unlisten = listen<PlayerState>("player-state", (event) => {
      if (seeking) return;

      const s = event.payload;
      if (seekTarget.current !== null) {
        if (Math.abs(s.position - seekTarget.current) < 1.0) {
          seekTarget.current = null;
        } else {
          s.position = seekTarget.current;
        }
      }
      setState(s);
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

  // Progress bar hover → compute time at cursor position
  const handleProgressHover = (e: React.MouseEvent) => {
    if (!progressRef.current || state.duration <= 0) return;
    const rect = progressRef.current.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    setHoverTime({ x: e.clientX - rect.left, time: ratio * state.duration });
  };

  return (
    <div
      style={{
        position: "fixed", inset: 0,
        background: "transparent",
        display: "flex", flexDirection: "column",
        cursor: showControls ? "default" : "none",
        fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif",
        userSelect: "none",
      }}
      onMouseMove={resetHideTimer}
    >
      {/* Click area — play/pause on click, fullscreen on double-click */}
      <div style={{ flex: 1 }} onClick={() => playPause()} onDoubleClick={() => toggleFullscreen()} />

      {/* Track selector panel */}
      {showTracks && (
        <div style={{
          position: "absolute", right: 24, bottom: 100,
          background: glass.bg,
          backdropFilter: glass.backdrop,
          WebkitBackdropFilter: glass.backdrop,
          border: glass.border,
          borderRadius: glass.radius,
          padding: "6px 0", minWidth: 200,
          boxShadow: glass.shadow,
        }}>
          {["audio", "sub"].map((type) => {
            const items = tracks.filter((t) => t.track_type === type);
            if (items.length === 0) return null;
            return (
              <div key={type}>
                <div style={{
                  padding: "8px 16px 4px", fontSize: 11, fontWeight: 600,
                  color: glass.textDim, textTransform: "uppercase", letterSpacing: "0.06em",
                }}>
                  {type === "audio" ? "音轨" : "字幕"}
                </div>
                {type === "sub" && (
                  <TrackItem
                    label="关闭"
                    selected={tracks.every((t) => t.track_type !== "sub" || !t.selected)}
                    onClick={() => { setSubTrack(0); setShowTracks(false); }}
                  />
                )}
                {items.map((t) => (
                  <TrackItem
                    key={t.id}
                    label={trackLabel(t)}
                    selected={t.selected}
                    onClick={() => { (type === "audio" ? setAudioTrack : setSubTrack)(t.id); setShowTracks(false); }}
                  />
                ))}
              </div>
            );
          })}
        </div>
      )}

      {/* ── Controls bar (liquid glass) ────────────────────────── */}
      <div style={{
        margin: "0 16px 16px",
        padding: "12px 16px 14px",
        background: glass.bg,
        backdropFilter: glass.backdrop,
        WebkitBackdropFilter: glass.backdrop,
        border: glass.border,
        borderRadius: glass.radius,
        boxShadow: glass.shadow,
        opacity: showControls ? 1 : 0,
        transition: "opacity 0.3s ease",
        pointerEvents: showControls ? "auto" : "none",
      }}>
        {/* Progress bar */}
        <div
          ref={progressRef}
          style={{
            height: 4, background: glass.trackBg, borderRadius: 2,
            cursor: "pointer", marginBottom: 12, position: "relative",
            transition: "height 0.15s ease",
          }}
          onMouseEnter={(e) => { (e.currentTarget as HTMLElement).style.height = "6px"; }}
          onMouseLeave={(e) => {
            if (!seeking) (e.currentTarget as HTMLElement).style.height = "4px";
            setHoverTime(null);
          }}
          onMouseMove={handleProgressHover}
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
              const targetPos = ratio * state.duration;
              seekTarget.current = targetPos;
              seek(targetPos);
              setState(prev => ({ ...prev, position: targetPos }));
              setSeeking(false);
              bar.style.height = "4px";
              window.removeEventListener("mousemove", onMove);
              window.removeEventListener("mouseup", onUp);
            };
            window.addEventListener("mousemove", onMove);
            window.addEventListener("mouseup", onUp);
          }}
        >
          {/* Fill */}
          <div style={{
            position: "absolute", inset: 0,
            borderRadius: 2, background: glass.trackFill,
            width: `${progress}%`, transition: seeking ? "none" : "width 0.1s linear",
          }} />
          {/* Hover time tooltip */}
          {hoverTime && !seeking && (
            <div style={{
              position: "absolute",
              left: hoverTime.x, bottom: 14,
              transform: "translateX(-50%)",
              background: glass.bg,
              backdropFilter: glass.backdrop,
              WebkitBackdropFilter: glass.backdrop,
              border: glass.borderLight,
              borderRadius: 6,
              padding: "3px 8px",
              fontSize: 11, fontWeight: 500,
              color: glass.text,
              fontVariantNumeric: "tabular-nums",
              whiteSpace: "nowrap",
              pointerEvents: "none",
              boxShadow: glass.shadowSmall,
            }}>
              {formatTime(hoverTime.time)}
            </div>
          )}
        </div>

        {/* Button row */}
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {/* Play/Pause */}
          <GlassButton onClick={() => playPause()} size={36}>
            {state.paused ? <PlayIcon /> : <PauseIcon />}
          </GlassButton>

          {/* Volume */}
          <div style={{ display: "flex", alignItems: "center", gap: 6, marginLeft: 4 }}>
            <GlassButton onClick={() => setVolume(state.volume > 0 ? 0 : 100)} size={30}>
              <VolumeIcon level={state.volume} />
            </GlassButton>
            <VolumeSlider value={state.volume} onChange={setVolume} />
          </div>

          {/* Time */}
          <span style={{
            fontSize: 12, color: glass.textDim, fontVariantNumeric: "tabular-nums",
            marginLeft: 8,
          }}>
            {formatTime(seeking ? seekPos : state.position)}
            <span style={{ margin: "0 3px", opacity: 0.4 }}>/</span>
            {formatTime(state.duration)}
          </span>

          <div style={{ flex: 1 }} />

          {/* Title */}
          <span style={{
            fontSize: 12, color: glass.textDim,
            maxWidth: 300, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
          }}>
            {state.title}
          </span>

          {/* Tracks */}
          <GlassButton
            onClick={() => showTracks ? setShowTracks(false) : loadTracks()}
            size={30} title="字幕/音轨"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <rect x="2" y="3" width="20" height="18" rx="2" />
              <path d="M7 15h4m-4-3h10m-10-3h6" />
            </svg>
          </GlassButton>

          {/* Fullscreen */}
          <GlassButton onClick={() => toggleFullscreen()} size={30} title="全屏">
            <FullscreenIcon />
          </GlassButton>
        </div>
      </div>
    </div>
  );
}

// ── Shared components ───────────────────────────────────────────

function GlassButton({ onClick, children, size = 30, title }: {
  onClick: () => void;
  children: React.ReactNode;
  size?: number;
  title?: string;
}) {
  const [hover, setHover] = useState(false);
  const [active, setActive] = useState(false);

  return (
    <button
      onClick={onClick}
      title={title}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => { setHover(false); setActive(false); }}
      onMouseDown={() => setActive(true)}
      onMouseUp={() => setActive(false)}
      style={{
        background: active ? glass.bgActive : hover ? glass.bgHover : "transparent",
        border: "none",
        color: glass.text,
        cursor: "pointer",
        width: size, height: size,
        borderRadius: glass.radiusSmall,
        lineHeight: 0,
        display: "flex", alignItems: "center", justifyContent: "center",
        transition: "background 0.15s ease",
      }}
    >
      {children}
    </button>
  );
}

function TrackItem({ label, selected, onClick }: {
  label: string;
  selected: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);

  return (
    <div
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: "7px 16px", fontSize: 13, cursor: "pointer",
        color: selected ? "#fff" : "rgba(255,255,255,0.6)",
        background: hover ? glass.bgHover : "transparent",
        transition: "background 0.12s ease",
      }}
    >
      {selected && <span style={{ marginRight: 6 }}>&#10003;</span>}
      {label}
    </div>
  );
}
