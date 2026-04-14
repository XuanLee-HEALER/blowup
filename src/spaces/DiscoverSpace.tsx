import { useLocation, useNavigate } from "react-router-dom";
import { SegmentedControl } from "@mantine/core";
import { SpaceShell } from "../layout/SpaceShell";
import Search from "../pages/Search";
import Download from "../pages/Download";

type DiscoverTab = "search" | "downloads";

/**
 * Discover space — combines TMDB search and download queue under a single
 * top-level icon (formerly two separate sidebar entries). The tab switcher
 * lives in the toolbar and toggles between the two sub-views via routes.
 *
 * Phase C scope: tab + sub-view rendering. The legacy Search and Download
 * pages render their full content (including their own headers/search inputs)
 * inside the main area. Migrating the search input itself into the toolbar
 * is a future polish step (docs §104, §116).
 */
export function DiscoverSpace() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const activeTab: DiscoverTab = pathname === "/discover/downloads" ? "downloads" : "search";

  const toolbarLeft = (
    <SegmentedControl
      size="xs"
      value={activeTab}
      onChange={(v) => {
        if (v === "downloads") navigate("/discover/downloads");
        else navigate("/discover");
      }}
      data={[
        { label: "搜索", value: "search" },
        { label: "下载队列", value: "downloads" },
      ]}
    />
  );

  return (
    <SpaceShell
      toolbarLeft={toolbarLeft}
      main={activeTab === "search" ? <Search /> : <Download />}
    />
  );
}
