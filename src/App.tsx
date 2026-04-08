// src/App.tsx
import { useEffect, useState } from "react";
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import { NavItem } from "./components/ui/NavItem";
import Search from "./pages/Search";
import Settings from "./pages/Settings";
import Wiki from "./pages/Wiki";
import Graph from "./pages/Graph";
import Library from "./pages/Library";
import Download from "./pages/Download";
import Darkroom from "./pages/Darkroom";
import { MusicPlayer } from "./components/MusicPlayer";
import { config, type MusicTrack } from "./lib/tauri";

const NAV_SECTIONS = [
  {
    label: null,
    items: [{ icon: "⌕", label: "搜索", path: "/" }],
  },
  {
    label: "知识库",
    items: [
      { icon: "◎", label: "Wiki", path: "/wiki" },
      { icon: "⋯", label: "知识图谱", path: "/graph" },
    ],
  },
  {
    label: "电影库",
    items: [
      { icon: "⊞", label: "影片", path: "/library" },
      { icon: "↓", label: "下载", path: "/download" },
      { icon: "◑", label: "暗房", path: "/darkroom" },
    ],
  },
];

const KB_PATHS = ["/", "/wiki", "/graph", "/library"];

export default function App() {
  const navigate = useNavigate();
  const { pathname } = useLocation();

  const isKbActive = KB_PATHS.some(
    (p) => pathname === p || (p !== "/" && pathname.startsWith(p + "/"))
  );

  const [musicEnabled, setMusicEnabled] = useState(false);
  const [musicMode, setMusicMode] = useState<"sequential" | "random">("sequential");
  const [musicPlaylist, setMusicPlaylist] = useState<MusicTrack[]>([]);

  useEffect(() => {
    console.log("[blowup] App mounted", performance.now().toFixed(0) + "ms");
    config.get().then((cfg) => {
      console.log("[blowup] config loaded", performance.now().toFixed(0) + "ms");
      if (cfg.music) {
        setMusicEnabled(!!cfg.music.enabled);
        setMusicMode(cfg.music.mode === "random" ? "random" : "sequential");
        setMusicPlaylist(cfg.music.playlist ?? []);
      }
    }).catch((e) => console.error("[blowup] config.get failed:", e));
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh", overflow: "hidden" }}>
      <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>
        {/* Sidebar */}
        <aside
          style={{
            width: 188,
            flexShrink: 0,
            background: "var(--color-bg-secondary)",
            borderRight: "1px solid var(--color-separator)",
            display: "flex",
            flexDirection: "column",
            padding: "1rem 0.5rem",
            gap: 1,
          }}
        >
          {NAV_SECTIONS.map((section, si) => (
            <div key={si}>
              {section.label && (
                <p
                  style={{
                    fontSize: "0.62rem",
                    color: "var(--color-label-quaternary)",
                    letterSpacing: "0.08em",
                    textTransform: "uppercase",
                    padding: "0.85rem 0.75rem 0.3rem",
                    margin: 0,
                  }}
                >
                  {section.label}
                </p>
              )}
              {section.items.map((item) => (
                <NavItem
                  key={item.path}
                  icon={item.icon}
                  label={item.label}
                  active={pathname === item.path}
                  disabled={"disabled" in item && (item.disabled as boolean)}
                  onClick={() => navigate(item.path)}
                />
              ))}
            </div>
          ))}

          {/* Bottom: Settings */}
          <div
            style={{
              marginTop: "auto",
              borderTop: "1px solid var(--color-separator)",
              paddingTop: "0.5rem",
            }}
          >
            <NavItem
              icon="⚙"
              label="设置"
              active={pathname === "/settings"}
              onClick={() => navigate("/settings")}
            />
          </div>
        </aside>

        {/* Content */}
        <main style={{ flex: 1, overflow: "hidden", display: "flex", flexDirection: "column" }}>
          <Routes>
            <Route path="/" element={<Search />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/wiki"     element={<Wiki />} />
            <Route path="/graph"    element={<Graph />} />
            <Route path="/library"  element={<Library />} />
            <Route path="/download" element={<Download />} />
            <Route path="/darkroom" element={<Darkroom />} />
          </Routes>
        </main>
      </div>

      <MusicPlayer
        active={isKbActive}
        enabled={musicEnabled}
        mode={musicMode}
        playlist={musicPlaylist}
      />
    </div>
  );
}
