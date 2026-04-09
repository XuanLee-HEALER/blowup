// src/pages/Graph.tsx
import { useEffect, useRef, useState, useCallback } from "react";
import * as d3 from "d3";
import { kb } from "../lib/tauri";
import type { GraphData, GraphNode } from "../lib/tauri";

interface SimNode extends GraphNode {
  x?: number; y?: number; vx?: number; vy?: number;
  fx?: number | null; fy?: number | null;
}

interface SimLink { source: SimNode; target: SimNode; relation_type: string; }

const nodeRadius = (n: SimNode) => 6 + n.weight * 5;
const nodeFill = () => "#007AFF";
const nodeStroke = () => "rgba(0,122,255,0.3)";

export default function Graph() {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [data, setData] = useState<GraphData | null>(null);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<SimNode | null>(null);

  useEffect(() => { kb.getGraphData().then(setData).catch(console.error); }, []);

  const buildGraph = useCallback(() => {
    if (!data || !svgRef.current) return;
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();
    const width = svgRef.current.clientWidth;
    const height = svgRef.current.clientHeight;
    const g = svg.append("g");

    svg.call(
      d3.zoom<SVGSVGElement, unknown>().scaleExtent([0.2, 4])
        .on("zoom", (e) => g.attr("transform", e.transform))
    );

    const nodes: SimNode[] = data.nodes.map((n) => ({ ...n }));
    const nodeById = new Map(nodes.map((n) => [n.id, n]));

    const links: SimLink[] = data.links
      .map((l) => ({ source: nodeById.get(l.source)!, target: nodeById.get(l.target)!, relation_type: l.relation_type }))
      .filter((l) => l.source && l.target);

    const simulation = d3.forceSimulation<SimNode>(nodes)
      .force("link", d3.forceLink<SimNode, SimLink>(links).id((d) => d.id).distance(120))
      .force("charge", d3.forceManyBody().strength(-200))
      .force("center", d3.forceCenter(width / 2, height / 2))
      .force("collide", d3.forceCollide<SimNode>().radius((d) => nodeRadius(d) + 8))
      .alphaDecay(0.02);

    // Links
    const link = g.append("g").selectAll("line").data(links).join("line")
      .attr("stroke", "rgba(255,255,255,0.12)").attr("stroke-width", 1);

    // Link labels (relation type)
    const linkLabel = g.append("g").selectAll("text").data(links).join("text")
      .text((d) => d.relation_type)
      .attr("font-size", 8)
      .attr("fill", "rgba(255,255,255,0.3)")
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
    const label = g.append("g").selectAll("text").data(nodes).join("text")
      .text((d) => d.label)
      .attr("font-size", 10)
      .attr("fill", "rgba(255,255,255,0.7)").attr("text-anchor", "middle")
      .attr("dy", (d) => nodeRadius(d) + 13)
      .style("pointer-events", "none").style("user-select", "none");

    simulation.on("tick", () => {
      link.attr("x1", (d) => d.source.x ?? 0).attr("y1", (d) => d.source.y ?? 0)
          .attr("x2", (d) => d.target.x ?? 0).attr("y2", (d) => d.target.y ?? 0);
      linkLabel
        .attr("x", (d) => ((d.source.x ?? 0) + (d.target.x ?? 0)) / 2)
        .attr("y", (d) => ((d.source.y ?? 0) + (d.target.y ?? 0)) / 2 - 4);
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
      svg.selectAll("circle, line, text").attr("opacity", 1);
      return;
    }
    const connected = new Set([hoveredId]);
    data.links.forEach((l) => {
      if (l.source === hoveredId) connected.add(l.target as string);
      if (l.target === hoveredId) connected.add(l.source as string);
    });
    svg.selectAll<SVGCircleElement, SimNode>("circle").attr("opacity", (d) => connected.has(d.id) ? 1 : 0.15);
    svg.selectAll<SVGLineElement, SimLink>("line").attr("opacity", (d) =>
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
    <div style={{ position: "relative", width: "100%", height: "100%", background: "var(--color-bg-primary)" }}>
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
          if (!svgRef.current) return;
          d3.select(svgRef.current).transition().duration(500)
            .call((d3.zoom() as d3.ZoomBehavior<SVGSVGElement, unknown>).transform, d3.zoomIdentity);
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
