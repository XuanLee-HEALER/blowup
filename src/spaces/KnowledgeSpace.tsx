import { useLocation, useNavigate } from "react-router-dom";
import { SegmentedControl } from "@mantine/core";
import { SpaceShell } from "../layout/SpaceShell";
import Wiki from "../pages/Wiki";
import Graph from "../pages/Graph";

type KnowledgeView = "list" | "graph";

/**
 * Knowledge space — list view (Wiki) and graph view (D3 force layout)
 * are two views over the same entries+relations data, switched via
 * SegmentedControl in the toolbar. Editing an entry is a separate
 * full-width route (`/knowledge/edit/:entryId`) handled by KnowledgeEditor.
 *
 * Phase D scope: top-level view switch only. The list and graph pages
 * keep their internal layouts (Wiki has its own list+detail split inside;
 * Graph is the full canvas). Future polish step: lift the Wiki entry
 * preview into the context panel and edit into a full-width overlay.
 */
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
