import { useLocation, useNavigate } from "react-router-dom";
import { SegmentedControl } from "@mantine/core";
import { SpaceShell } from "../layout/SpaceShell";
import Wiki from "../pages/Wiki";
import Graph from "../pages/Graph";

type KnowledgeView = "list" | "graph";

/** Knowledge space — list (Wiki) and graph (D3 force layout) views
 *  over the same entries+relations data, switched via the toolbar. */
export function KnowledgeSpace() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const activeView: KnowledgeView = pathname.startsWith("/knowledge/graph") ? "graph" : "list";

  const toolbarLeft = (
    <SegmentedControl
      size="xs"
      value={activeView}
      onChange={(v) => {
        if (v === "graph") navigate("/knowledge/graph");
        else navigate("/knowledge");
      }}
      data={[
        { label: "列表", value: "list" },
        { label: "图谱", value: "graph" },
      ]}
    />
  );

  return (
    <SpaceShell
      toolbarLeft={toolbarLeft}
      main={activeView === "list" ? <Wiki /> : <Graph />}
    />
  );
}
