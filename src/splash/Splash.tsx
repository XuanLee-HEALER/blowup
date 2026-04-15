import { useEffect, useRef } from "react";
import { animate, createTimeline, stagger, utils } from "animejs";
import "./Splash.css";

const DEBUG_LOOP = false;
const PARTICLE_COUNT = 30;

export default function Splash({
  onComplete,
  className = "",
}: {
  onComplete?: () => void;
  className?: string;
}) {
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;

    const reelAnim = animate("#reel-left, #reel-right", {
      rotate: "1turn",
      duration: 2400,
      ease: "linear",
      loop: true,
    });

    const tl = createTimeline({
      loop: DEBUG_LOOP,
      defaults: { ease: "outQuad" },
      onComplete: () => {
        onComplete?.();
      },
    });

    tl
      .add(
        "#film-in",
        {
          x: [0, -200],
          duration: 1800,
          ease: "linear",
        },
        0,
      )
      .add(
        ".particle",
        {
          x: () => utils.random(-170, 40),
          y: () => utils.random(-130, 130),
          rotate: () => utils.random(-720, 720),
          scale: [
            { from: 0, to: 1.1, duration: 200 },
            { to: 0.2, duration: 900 },
          ],
          opacity: [
            { from: 0, to: 1, duration: 200 },
            { to: 0, duration: 900 },
          ],
          delay: stagger(26),
          ease: "outExpo",
        },
        1100,
      );

    return () => {
      reelAnim.pause();
      tl.pause();
    };
  }, [onComplete]);

  return (
    <div className={`splash-root ${className}`.trim()}>
      <svg
        ref={svgRef}
        className="splash-svg"
        viewBox="0 0 560 340"
        xmlns="http://www.w3.org/2000/svg"
        preserveAspectRatio="xMidYMid meet"
      >
        {/* 胶片（在 camera 之前，被机身遮住中段） */}
        <FilmStrip id="film-in" rectX={440} />

        <g id="camera" fill="#050505" stroke="#f0f0f0">
          {/* ── 三脚架 ────────────────────────── */}
          <g strokeWidth="2.4" strokeLinecap="round" fill="none">
            <line x1="320" y1="245" x2="254" y2="318" />
            <line x1="320" y1="245" x2="320" y2="324" />
            <line x1="320" y1="245" x2="386" y2="318" />
          </g>

          {/* ── 双胶卷盘 + 机身短轴 ─────────── */}
          <line x1="296" y1="146" x2="296" y2="150" strokeWidth="3" strokeLinecap="round" />
          <line x1="344" y1="146" x2="344" y2="150" strokeWidth="3" strokeLinecap="round" />
          <Reel id="reel-left" cx={296} cy={118} r={28} />
          <Reel id="reel-right" cx={344} cy={118} r={28} />

          {/* ── 机身 ─────────────────────────── */}
          <rect x="228" y="150" width="184" height="95" strokeWidth="2.6" />

          {/* 机身下半三个旋钮（极简符号） */}
          <circle cx="268" cy="228" r="4" strokeWidth="1.8" />
          <circle cx="292" cy="228" r="4" strokeWidth="1.8" />
          <circle cx="316" cy="228" r="4" strokeWidth="1.8" />

          {/* ── 取景器：机身右上水平细筒 + 目镜圆 ── */}
          <rect
            x="412"
            y="163"
            width="48"
            height="10"
            rx="3"
            strokeWidth="2.2"
          />
          <circle cx="466" cy="168" r="7" strokeWidth="2.2" />

          {/* ── 镜头（左侧 matte box） ──────── */}
          {/* 梯形本体（机身端窄，前端宽） */}
          <polygon
            points="228,184 228,222 150,168 150,238"
            strokeWidth="2.6"
            strokeLinejoin="round"
          />
          {/* 前端外唇（遮光罩前框） */}
          <rect x="136" y="168" width="14" height="70" strokeWidth="2.6" />
          {/* 内部镜头本体（梯形里隐约可见的实际镜头） */}
          <circle cx="202" cy="203" r="9" strokeWidth="1.8" />
          <circle cx="202" cy="203" r="2.5" fill="#f0f0f0" stroke="none" />
        </g>

        {/* ── 爆炸粒子（从镜头前端外唇喷出） ── */}
        <g transform="translate(124, 203)">
          {Array.from({ length: PARTICLE_COUNT }).map((_, i) => (
            <rect
              key={i}
              className="particle"
              x="-8"
              y="-5"
              width="16"
              height="10"
              fill="#050505"
              stroke="#f0f0f0"
              strokeWidth="1"
            />
          ))}
        </g>
      </svg>
    </div>
  );
}

// ─────────────────────────────────────────────

function Reel({ id, cx, cy, r }: { id: string; cx: number; cy: number; r: number }) {
  const spokes = [0, 45, 90, 135, 180, 225, 270, 315];
  return (
    <g id={id} style={{ transformOrigin: "center", transformBox: "fill-box" }}>
      <circle cx={cx} cy={cy} r={r} fill="#050505" stroke="#f0f0f0" strokeWidth="2.4" />
      {spokes.map((a) => {
        const rad = (a * Math.PI) / 180;
        return (
          <line
            key={a}
            x1={cx}
            y1={cy}
            x2={cx + (r - 3) * Math.cos(rad)}
            y2={cy + (r - 3) * Math.sin(rad)}
            stroke="#f0f0f0"
            strokeWidth="1.2"
          />
        );
      })}
      <circle cx={cx} cy={cy} r={4} fill="#f0f0f0" />
    </g>
  );
}

function FilmStrip({ id, rectX }: { id: string; rectX: number }) {
  const width = 160;
  const holeCount = 10;
  const holeSpacing = 15;
  const holeStartOffset = 8;
  const filmY = 190;
  const height = 26;
  const stripH = 5;
  return (
    <g id={id}>
      <rect x={rectX} y={filmY} width={width} height={height} fill="#050505" />
      <rect x={rectX} y={filmY + 1} width={width} height={stripH} fill="#f0f0f0" />
      {Array.from({ length: holeCount }).map((_, i) => (
        <rect
          key={`t${i}`}
          x={rectX + holeStartOffset + i * holeSpacing}
          y={filmY + 2}
          width="8"
          height="3"
          fill="#050505"
        />
      ))}
      <rect
        x={rectX}
        y={filmY + height - stripH - 1}
        width={width}
        height={stripH}
        fill="#f0f0f0"
      />
      {Array.from({ length: holeCount }).map((_, i) => (
        <rect
          key={`b${i}`}
          x={rectX + holeStartOffset + i * holeSpacing}
          y={filmY + height - stripH}
          width="8"
          height="3"
          fill="#050505"
        />
      ))}
      {Array.from({ length: 4 }).map((_, i) => (
        <line
          key={`f${i}`}
          x1={rectX + 30 + i * 35}
          y1={filmY + stripH + 3}
          x2={rectX + 30 + i * 35}
          y2={filmY + height - stripH - 3}
          stroke="#f0f0f0"
          strokeWidth="0.6"
          opacity="0.4"
        />
      ))}
    </g>
  );
}
