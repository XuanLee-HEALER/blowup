// src/components/ReviewSection.tsx
import { useState } from "react";
import { library } from "../lib/tauri";
import type { ReviewEntry } from "../lib/tauri";

function RatingDots({ value, onChange }: { value: number | null; onChange: (v: number) => void }) {
  const [hovered, setHovered] = useState<number | null>(null);
  const display = hovered ?? value ?? 0;

  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.2rem", flexWrap: "wrap" }}>
      {Array.from({ length: 20 }, (_, i) => {
        const v = (i + 1) * 0.5;
        return (
          <button
            key={v}
            title={v.toFixed(1)}
            onMouseEnter={() => setHovered(v)}
            onMouseLeave={() => setHovered(null)}
            onClick={() => onChange(v)}
            style={{
              width: 11, height: 11, borderRadius: "50%", padding: 0, cursor: "pointer",
              border: "1px solid var(--color-accent)",
              background: display >= v ? "var(--color-accent)" : "transparent",
              flexShrink: 0, transition: "background 0.1s",
            }}
          />
        );
      })}
      {value !== null && (
        <span style={{ marginLeft: "0.4rem", fontSize: "0.8rem", color: "var(--color-accent)", fontWeight: 500 }}>
          {value.toFixed(1)} / 10
        </span>
      )}
    </div>
  );
}

function CriticCard({ review, onDelete }: { review: ReviewEntry; onDelete: () => void }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <div style={{ background: "var(--color-bg-elevated)", borderRadius: 6, padding: "0.65rem 0.75rem", fontSize: "0.78rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "0.3rem" }}>
        <span style={{ fontWeight: 500, color: "var(--color-label-secondary)" }}>{review.author ?? "佚名"}</span>
        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          {review.rating !== null && (
            <span style={{ color: "var(--color-accent)", fontSize: "0.72rem" }}>★ {review.rating.toFixed(1)}</span>
          )}
          <button onClick={onDelete} style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.7rem", padding: 0 }}>✕</button>
        </div>
      </div>
      <p style={{
        margin: 0, color: "var(--color-label-tertiary)", lineHeight: 1.55,
        display: "-webkit-box", WebkitLineClamp: expanded ? undefined : 3,
        WebkitBoxOrient: "vertical", overflow: expanded ? "visible" : "hidden",
      } as React.CSSProperties}>
        {review.content}
      </p>
      {review.content.length > 160 && (
        <button onClick={() => setExpanded(!expanded)} style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.7rem", padding: "0.2rem 0 0", fontFamily: "inherit" }}>
          {expanded ? "收起" : "展开"}
        </button>
      )}
    </div>
  );
}

interface ReviewSectionProps {
  filmId: number;
  reviews: ReviewEntry[];
  onRefresh: () => void;
}

export function ReviewSection({ filmId, reviews, onRefresh }: ReviewSectionProps) {
  const personal = reviews.find((r) => r.is_personal);
  const critics = reviews.filter((r) => !r.is_personal);

  const [personalContent, setPersonalContent] = useState(personal?.content ?? "");
  const [personalRating, setPersonalRating] = useState<number | null>(personal?.rating ?? null);
  const [showAddCritic, setShowAddCritic] = useState(false);
  const [criticAuthor, setCriticAuthor] = useState("");
  const [criticContent, setCriticContent] = useState("");
  const [criticRating, setCriticRating] = useState<number | null>(null);

  const savePersonal = async () => {
    if (!personalContent.trim()) return;
    if (personal) {
      await library.updateReview(personal.id, personalContent, personalRating);
    } else {
      await library.addReview(filmId, true, null, personalContent, personalRating);
    }
    onRefresh();
  };

  const addCritic = async () => {
    if (!criticContent.trim()) return;
    await library.addReview(filmId, false, criticAuthor.trim() || null, criticContent, criticRating);
    setCriticAuthor(""); setCriticContent(""); setCriticRating(null);
    setShowAddCritic(false);
    onRefresh();
  };

  const labelStyle: React.CSSProperties = {
    margin: "0 0 0.5rem", fontSize: "0.72rem",
    color: "var(--color-label-quaternary)",
    textTransform: "uppercase", letterSpacing: "0.06em",
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
      <section>
        <p style={labelStyle}>个人影评</p>
        <div style={{ marginBottom: "0.5rem" }}>
          <RatingDots value={personalRating} onChange={setPersonalRating} />
        </div>
        <div style={{ position: "relative" }}>
          <textarea
            value={personalContent}
            onChange={(e) => { if (e.target.value.length <= 500) setPersonalContent(e.target.value); }}
            onBlur={savePersonal}
            placeholder="写下你的想法…（最多 500 字）"
            style={{
              width: "100%", minHeight: 100, outline: "none", resize: "vertical",
              background: "var(--color-bg-elevated)",
              border: "1px solid var(--color-separator)",
              borderRadius: 6, padding: "0.6rem 0.75rem",
              color: "var(--color-label-primary)",
              fontSize: "0.8rem", fontFamily: "inherit", lineHeight: 1.6, boxSizing: "border-box",
            }}
          />
          <span style={{ position: "absolute", bottom: "0.4rem", right: "0.6rem", fontSize: "0.65rem", color: "var(--color-label-quaternary)" }}>
            {personalContent.length} / 500
          </span>
        </div>
      </section>

      <section>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.5rem" }}>
          <p style={{ ...labelStyle, margin: 0 }}>收录影评</p>
          <button onClick={() => setShowAddCritic(!showAddCritic)} style={{ background: "none", border: "none", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>
            + 添加
          </button>
        </div>

        {showAddCritic && (
          <div style={{ background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 6, padding: "0.75rem", marginBottom: "0.5rem", display: "flex", flexDirection: "column", gap: "0.5rem" }}>
            <input placeholder="作者姓名（可选）" value={criticAuthor} onChange={(e) => setCriticAuthor(e.target.value)}
              style={{ background: "var(--color-bg-primary)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.35rem 0.5rem", color: "var(--color-label-primary)", fontSize: "0.78rem", fontFamily: "inherit", outline: "none" }} />
            <RatingDots value={criticRating} onChange={setCriticRating} />
            <textarea placeholder="影评内容…" value={criticContent} onChange={(e) => setCriticContent(e.target.value)}
              style={{ background: "var(--color-bg-primary)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.35rem 0.5rem", color: "var(--color-label-primary)", fontSize: "0.78rem", fontFamily: "inherit", minHeight: 80, resize: "vertical", outline: "none" }} />
            <div style={{ display: "flex", gap: "0.5rem", justifyContent: "flex-end" }}>
              <button onClick={() => setShowAddCritic(false)} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>取消</button>
              <button onClick={addCritic} style={{ background: "var(--color-accent-soft)", border: "1px solid var(--color-accent)", borderRadius: 4, padding: "0.25rem 0.75rem", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>保存</button>
            </div>
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
          {critics.map((r) => (
            <CriticCard key={r.id} review={r} onDelete={async () => { await library.deleteReview(r.id); onRefresh(); }} />
          ))}
          {critics.length === 0 && !showAddCritic && (
            <p style={{ fontSize: "0.75rem", color: "var(--color-label-quaternary)", margin: 0 }}>暂无收录影评</p>
          )}
        </div>
      </section>
    </div>
  );
}
