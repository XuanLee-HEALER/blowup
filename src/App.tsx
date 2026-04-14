// src/App.tsx
import { useEffect, useState, useCallback } from "react";
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import { AppShell, Box, NavLink, Stack, Text } from "@mantine/core";
import Search from "./pages/Search";
import Settings from "./pages/Settings";
import Wiki from "./pages/Wiki";
import Graph from "./pages/Graph";
import Library from "./pages/Library";
import Download from "./pages/Download";
import Darkroom from "./pages/Darkroom";
import { MusicPlayer } from "./components/MusicPlayer";
import { config, type MusicTrack } from "./lib/tauri";
import { useBackendEvent, BackendEvent } from "./lib/useBackendEvent";

interface NavEntry {
  icon: string;
  label: string;
  path: string;
}

interface NavSection {
  label: string | null;
  items: NavEntry[];
}

const NAV_SECTIONS: NavSection[] = [
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

function NavIcon({ children }: { children: string }) {
  return (
    <Text component="span" ta="center" w={15} fz="sm">
      {children}
    </Text>
  );
}

export default function App() {
  const navigate = useNavigate();
  const { pathname } = useLocation();

  const isKbActive = KB_PATHS.some(
    (p) => pathname === p || (p !== "/" && pathname.startsWith(p + "/"))
  );

  const [musicEnabled, setMusicEnabled] = useState(false);
  const [musicMode, setMusicMode] = useState<"sequential" | "random">("sequential");
  const [musicPlaylist, setMusicPlaylist] = useState<MusicTrack[]>([]);

  const loadMusicConfig = useCallback(() => {
    config.get().then((cfg) => {
      if (cfg.music) {
        setMusicEnabled(!!cfg.music.enabled);
        setMusicMode(cfg.music.mode === "random" ? "random" : "sequential");
        setMusicPlaylist(cfg.music.playlist ?? []);
      }
    }).catch((e) => console.error("[blowup] config.get failed:", e));
  }, []);

  useEffect(() => {
    console.log("[blowup] App mounted", performance.now().toFixed(0) + "ms");
    loadMusicConfig();
  }, [loadMusicConfig]);

  useBackendEvent(BackendEvent.CONFIG_CHANGED, loadMusicConfig);

  return (
    <AppShell
      navbar={{ width: 188, breakpoint: 0 }}
      padding={0}
      styles={{
        navbar: {
          background: "var(--color-bg-secondary)",
          borderRight: "1px solid var(--color-separator)",
          padding: "1rem 0.5rem",
        },
        main: {
          background: "var(--color-bg-primary)",
        },
      }}
    >
      <AppShell.Navbar>
        <Stack gap={1} h="100%">
          {NAV_SECTIONS.map((section, si) => (
            <Box key={si}>
              {section.label && (
                <Text
                  size="xs"
                  fw={500}
                  tt="uppercase"
                  c="var(--color-label-quaternary)"
                  pt="0.85rem"
                  pb="0.3rem"
                  px="0.75rem"
                  style={{ letterSpacing: "0.08em", fontSize: "0.62rem" }}
                >
                  {section.label}
                </Text>
              )}
              {section.items.map((item) => (
                <NavLink
                  key={item.path}
                  active={pathname === item.path}
                  label={item.label}
                  leftSection={<NavIcon>{item.icon}</NavIcon>}
                  onClick={() => navigate(item.path)}
                  variant="subtle"
                  styles={{
                    root: {
                      borderRadius: 6,
                      padding: "0.42rem 0.75rem",
                      fontSize: "0.82rem",
                    },
                    label: { fontSize: "0.82rem" },
                  }}
                />
              ))}
            </Box>
          ))}

          {/* Bottom: Settings */}
          <Box
            mt="auto"
            pt="0.5rem"
            style={{ borderTop: "1px solid var(--color-separator)" }}
          >
            <NavLink
              active={pathname === "/settings"}
              label="设置"
              leftSection={<NavIcon>⚙</NavIcon>}
              onClick={() => navigate("/settings")}
              variant="subtle"
              styles={{
                root: {
                  borderRadius: 6,
                  padding: "0.42rem 0.75rem",
                  fontSize: "0.82rem",
                },
                label: { fontSize: "0.82rem" },
              }}
            />
          </Box>
        </Stack>
      </AppShell.Navbar>

      <AppShell.Main h="100vh" style={{ overflow: "hidden", display: "flex", flexDirection: "column" }}>
        <Box style={{ flex: 1, overflow: "hidden", display: "flex", flexDirection: "column" }}>
          <Routes>
            <Route path="/" element={<Search />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/wiki"     element={<Wiki />} />
            <Route path="/graph"    element={<Graph />} />
            <Route path="/library"  element={<Library />} />
            <Route path="/download" element={<Download />} />
            <Route path="/darkroom" element={<Darkroom />} />
          </Routes>
        </Box>

        <MusicPlayer
          active={isKbActive}
          enabled={musicEnabled}
          mode={musicMode}
          playlist={musicPlaylist}
        />
      </AppShell.Main>
    </AppShell>
  );
}
