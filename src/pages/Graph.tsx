// src/pages/Graph.tsx
import { useEffect, useRef, useState, useCallback } from "react";
import * as d3 from "d3";
import { kb } from "../lib/tauri";
import type { GraphData, GraphNode } from "../lib/tauri";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

interface SimNode extends GraphNode {
  x?: number; y?: number; vx?: number; vy?: number;
  fx?: number | null; fy?: number | null;
}

interface SimLink { source: SimNode; target: SimNode; relation_type: string; index: number; linkCount: number; }

const ARROW_SIZE = 7;
const nodeRadius = (n: SimNode) => 6 + n.weight * 5;
const nodeFill = () => "var(--color-accent)";
const nodeStroke = () => "var(--color-accent-soft)";
const linkColor = "var(--color-label-quaternary)";
const linkLabelColor = "var(--color-label-tertiary)";
const labelColor = "var(--color-label-secondary)";

const gridBg: React.CSSProperties = {
  backgroundImage:
    "linear-gradient(rgba(0,0,0,0.04) 1px, transparent 1px)," +
    "linear-gradient(90deg, rgba(0,0,0,0.04) 1px, transparent 1px)",
  backgroundSize: "16px 16px",
};

export default function Graph() {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const zoomRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null);
  const [data, setData] = useState<GraphData | null>(null);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<SimNode | null>(null);

  const fetchGraph = useCallback(() => { kb.getGraphData().then(setData).catch(console.error); }, []);
  useEffect(fetchGraph, [fetchGraph]);
  useBackendEvent(BackendEvent.ENTRIES_CHANGED, fetchGraph);

  const buildGraph = useCallback(() => {
    if (!data || !svgRef.current) return;
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();
    const width = svgRef.current.clientWidth;
    const height = svgRef.current.clientHeight;

    // Arrow marker
    const defs = svg.append("defs");
    defs.append("marker")
      .attr("id", "arrow")
      .attr("viewBox", "0 0 10 6")
      .attr("refX", 10).attr("refY", 3)
      .attr("markerWidth", ARROW_SIZE).attr("markerHeight", ARROW_SIZE)
      .attr("orient", "auto")
      .append("path")
      .attr("d", "M0,0 L10,3 L0,6 z")
      .attr("fill", "var(--color-label-tertiary)");

    const g = svg.append("g");

    const zoom = d3.zoom<SVGSVGElement, unknown>().scaleExtent([0.2, 4])
      .on("zoom", (e) => g.attr("transform", e.transform));
    svg.call(zoom);
    zoomRef.current = zoom;

    const nodes: SimNode[] = data.nodes.map((n) => ({ ...n }));
    const nodeById = new Map(nodes.map((n) => [n.id, n]));

    // Build links — count per unordered pair for curve offsets
    const pairCount = new Map<string, number>();
    const pairIndex = new Map<string, number>();
    const rawLinks = data.links
      .map((l) => ({ source: nodeById.get(l.source)!, target: nodeById.get(l.target)!, relation_type: l.relation_type }))
      .filter((l) => l.source && l.target);

    for (const l of rawLinks) {
      const key = [l.source.id, l.target.id].sort().join("-");
      pairCount.set(key, (pairCount.get(key) ?? 0) + 1);
    }

    const links: SimLink[] = rawLinks.map((l) => {
      const key = [l.source.id, l.target.id].sort().join("-");
      const idx = pairIndex.get(key) ?? 0;
      pairIndex.set(key, idx + 1);
      return { ...l, index: idx, linkCount: pairCount.get(key) ?? 1 };
    });

    const simulation = d3.forceSimulation<SimNode>(nodes)
      .force("link", d3.forceLink<SimNode, SimLink>(links).id((d) => d.id).distance(140))
      .force("charge", d3.forceManyBody().strength(-200))
      .force("center", d3.forceCenter(width / 2, height / 2))
      .force("collide", d3.forceCollide<SimNode>().radius((d) => nodeRadius(d) + 8))
      .alphaDecay(0.02);

    // Links — directed paths with arrows
    const link = g.append("g").selectAll<SVGPathElement, SimLink>("path").data(links).join("path")
      .attr("class", "graph-link")
      .attr("fill", "none")
      .attr("stroke", linkColor)
      .attr("stroke-width", 1.2)
      .attr("stroke-opacity", 0.6)
      .attr("marker-end", "url(#arrow)");

    // Link labels
    const linkLabel = g.append("g").selectAll<SVGTextElement, SimLink>("text").data(links).join("text")
      .text((d) => d.relation_type)
      .attr("font-size", 9)
      .attr("fill", linkLabelColor)
      .attr("text-anchor", "middle")
      .style("pointer-events", "none").style("user-select", "none");

    // Nodes
    const node = g.append("g").selectAll<SVGCircleElement, SimNode>("circle")
      .data(nodes).join("circle")
      .attr("r", nodeRadius).attr("fill", nodeFill)
      .attr("stroke", nodeStroke).attr("stroke-width", 1.5)
      .style("cursor", "pointer")
      .call(
        d3.drag<SVGCircleElement, SimNode>()
          .on("start", (e, d) => { if (!e.active) simulation.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; })
          .on("drag", (e, d) => { d.fx = e.x; d.fy = e.y; })
          .on("end", (e, d) => { if (!e.active) simulation.alphaTarget(0); d.fx = null; d.fy = null; })
      )
      .on("mouseenter", (_, d) => setHoveredId(d.id))
      .on("mouseleave", () => setHoveredId(null))
      .on("click", (_, d) => setSelectedNode((prev) => prev?.id === d.id ? null : d));

    // Node labels
    const label = g.append("g").selectAll<SVGTextElement, SimNode>("text").data(nodes).join("text")
      .text((d) => d.label)
      .attr("font-size", 10)
      .attr("fill", labelColor).attr("text-anchor", "middle")
      .attr("dy", (d) => nodeRadius(d) + 13)
      .style("pointer-events", "none").style("user-select", "none");

    // ── Path geometry helpers ────────────────────────────────────
    const SPREAD = 30;

    /** Shorten a vector (dx,dy) by `amount` from the tip, return new tip. */
    function shorten(ox: number, oy: number, dx: number, dy: number, amount: number): [number, number] {
      const d = Math.sqrt(dx * dx + dy * dy) || 1;
      return [ox + dx - (dx / d) * amount, oy + dy - (dy / d) * amount];
    }

    /** Canonical perpendicular: always computed from the smaller-id node
     *  to the larger-id node so A→B and B→A share the same normal direction. */
    function canonicalNormal(dd: SimLink): [number, number] {
      const sx = dd.source.x ?? 0, sy = dd.source.y ?? 0;
      const tx = dd.target.x ?? 0, ty = dd.target.y ?? 0;
      // Always go from smaller id to larger id
      const flip = dd.source.id > dd.target.id;
      const cdx = flip ? sx - tx : tx - sx;
      const cdy = flip ? sy - ty : ty - sy;
      const dist = Math.sqrt(cdx * cdx + cdy * cdy) || 1;
      return [-cdy / dist, cdx / dist];
    }

    function linkPath(dd: SimLink) {
      const sx = dd.source.x ?? 0, sy = dd.source.y ?? 0;
      const tx = dd.target.x ?? 0, ty = dd.target.y ?? 0;
      const sr = nodeRadius(dd.source);
      const tr = nodeRadius(dd.target) + ARROW_SIZE;
      const dx = tx - sx, dy = ty - sy;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;

      if (dd.linkCount <= 1) {
        const ux = dx / dist, uy = dy / dist;
        const x1 = sx + ux * sr, y1 = sy + uy * sr;
        const x2 = tx - ux * tr, y2 = ty - uy * tr;
        return `M${x1},${y1}L${x2},${y2}`;
      }

      const [nx, ny] = canonicalNormal(dd);
      const offset = (dd.index - (dd.linkCount - 1) / 2) * SPREAD;
      const cx = (sx + tx) / 2 + nx * offset;
      const cy = (sy + ty) / 2 + ny * offset;

      const [x1, y1] = shorten(sx, sy, cx - sx, cy - sy, sr);
      const [x2, y2] = shorten(cx, cy, tx - cx, ty - cy, tr);

      return `M${x1},${y1}Q${cx},${cy} ${x2},${y2}`;
    }

    function linkMid(dd: SimLink): [number, number] {
      const sx = dd.source.x ?? 0, sy = dd.source.y ?? 0;
      const tx = dd.target.x ?? 0, ty = dd.target.y ?? 0;
      if (dd.linkCount <= 1) {
        return [(sx + tx) / 2, (sy + ty) / 2];
      }
      const [nx, ny] = canonicalNormal(dd);
      const offset = (dd.index - (dd.linkCount - 1) / 2) * SPREAD;
      const cx = (sx + tx) / 2 + nx * offset;
      const cy = (sy + ty) / 2 + ny * offset;
      return [0.25 * sx + 0.5 * cx + 0.25 * tx, 0.25 * sy + 0.5 * cy + 0.25 * ty];
    }

    simulation.on("tick", () => {
      link.attr("d", linkPath);
      linkLabel
        .attr("x", (d) => linkMid(d)[0])
        .attr("y", (d) => linkMid(d)[1] - 4);
      node.attr("cx", (d) => d.x ?? 0).attr("cy", (d) => d.y ?? 0);
      label.attr("x", (d) => d.x ?? 0).attr("y", (d) => d.y ?? 0);
    });

    return () => { simulation.stop(); };
  }, [data]);

  useEffect(() => { const cleanup = buildGraph(); return cleanup; }, [buildGraph]);

  useEffect(() => {
    if (!svgRef.current || !data) return;
    const svg = d3.select(svgRef.current);
    if (!hoveredId) {
      svg.selectAll("circle, .graph-link, text").attr("opacity", 1);
      return;
    }
    const connected = new Set([hoveredId]);
    data.links.forEach((l) => {
      if (l.source === hoveredId) connected.add(l.target as string);
      if (l.target === hoveredId) connected.add(l.source as string);
    });
    svg.selectAll<SVGCircleElement, SimNode>("circle").attr("opacity", (d) => connected.has(d.id) ? 1 : 0.15);
    svg.selectAll<SVGPathElement, SimLink>(".graph-link").attr("opacity", (d) =>
      connected.has((d.source as SimNode).id) && connected.has((d.target as SimNode).id) ? 0.8 : 0.05
    );
    svg.selectAll<SVGTextElement, SimNode>("text").attr("opacity", (d) => {
      if ("id" in d) return connected.has(d.id) ? 1 : 0.1;
      return 0.1;
    });
  }, [hoveredId, data]);

  const toolbarBtnStyle: React.CSSProperties = {
    background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)",
    borderRadius: 6, padding: "0.35rem 0.65rem",
    color: "var(--color-label-secondary)", fontSize: "0.73rem",
    cursor: "pointer", fontFamily: "inherit", whiteSpace: "nowrap",
  };

  return (
    <div style={{ position: "relative", width: "100%", height: "100%", background: "var(--color-bg-primary)", ...gridBg }}>
      {data === null ? (
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>
          加载图谱数据…
        </div>
      ) : data.nodes.length === 0 ? (
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>
          知识库为空。先在 Wiki 中添加条目和关系。
        </div>
      ) : (
        <svg ref={svgRef} width="100%" height="100%" />
      )}

      {/* Toolbar */}
      <div style={{ position: "absolute", top: "1rem", right: "1rem", display: "flex", flexDirection: "column", gap: "0.4rem", zIndex: 10 }}>
        <button onClick={() => {
          if (!svgRef.current || !zoomRef.current) return;
          d3.select(svgRef.current).transition().duration(500)
            .call(zoomRef.current.transform, d3.zoomIdentity);
        }} style={toolbarBtnStyle}>重置视角</button>
      </div>

      {/* Selected node mini card */}
      {selectedNode && (
        <div style={{ position: "absolute", bottom: "1.5rem", right: "1rem", background: "var(--color-bg-secondary)", border: "1px solid var(--color-separator)", borderRadius: 8, padding: "0.85rem 1rem", width: 200, zIndex: 10 }}>
          <button onClick={() => setSelectedNode(null)} style={{ position: "absolute", top: "0.4rem", right: "0.5rem", background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.8rem" }}>✕</button>
          <p style={{ margin: 0, fontSize: "0.85rem", fontWeight: 600 }}>{selectedNode.label}</p>
          <p style={{ margin: "0.15rem 0 0", fontSize: "0.68rem", color: "var(--color-label-quaternary)" }}>
            关系数 {selectedNode.weight.toFixed(1)}
          </p>
        </div>
      )}
    </div>
  );
}
