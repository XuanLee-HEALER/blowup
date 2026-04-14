import { useLocation, useNavigate } from "react-router-dom";
import { SegmentedControl } from "@mantine/core";
import { SpaceShell } from "../layout/SpaceShell";
import Search from "../pages/Search";
import Download from "../pages/Download";

type DiscoverTab = "search" | "downloads";

/** Discover space — TMDB search + download queue under one icon, with
 *  a SegmentedControl in the toolbar to switch between the two. */
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
