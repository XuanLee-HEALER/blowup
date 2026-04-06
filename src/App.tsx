// src/App.tsx
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import { NavItem } from "./components/ui/NavItem";
import Search from "./pages/Search";
import Settings from "./pages/Settings";
import Placeholder from "./pages/Placeholder";

const NAV_SECTIONS = [
  {
    label: null,
    items: [{ icon: "⌕", label: "搜索", path: "/" }],
  },
  {
    label: "知识库",
    items: [
      { icon: "◎", label: "影人", path: "/people", disabled: true },
      { icon: "◈", label: "流派", path: "/genres", disabled: true },
      { icon: "⋯", label: "关系图", path: "/graph", disabled: true },
    ],
  },
  {
    label: "资源",
    items: [
      { icon: "⊞", label: "我的库", path: "/library", disabled: true },
      { icon: "↓", label: "下载", path: "/download", disabled: true },
    ],
  },
  {
    label: "工具",
    items: [
      { icon: "◷", label: "字幕", path: "/subtitle", disabled: true },
      { icon: "▶", label: "媒体", path: "/media", disabled: true },
    ],
  },
];

export default function App() {
  const navigate = useNavigate();
  const { pathname } = useLocation();

  return (
    <div style={{ display: "flex", height: "100vh", overflow: "hidden" }}>
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
                disabled={"disabled" in item && item.disabled}
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
          <Route path="/people"   element={<Placeholder title="影人" milestone="M2" />} />
          <Route path="/genres"   element={<Placeholder title="流派" milestone="M2" />} />
          <Route path="/graph"    element={<Placeholder title="关系图" milestone="M2" />} />
          <Route path="/library"  element={<Placeholder title="我的库" milestone="M3" />} />
          <Route path="/download" element={<Placeholder title="下载" milestone="M3" />} />
          <Route path="/subtitle" element={<Placeholder title="字幕" milestone="M4" />} />
          <Route path="/media"    element={<Placeholder title="媒体工具" milestone="M4" />} />
        </Routes>
      </main>
    </div>
  );
}
