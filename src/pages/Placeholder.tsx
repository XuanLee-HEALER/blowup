// src/pages/Placeholder.tsx
interface PlaceholderProps {
  title: string;
  milestone: string;
}

export default function Placeholder({ title, milestone }: PlaceholderProps) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        height: "100%",
        gap: "0.5rem",
      }}
    >
      <p style={{ color: "var(--color-label-tertiary)", fontSize: "1rem", fontWeight: 600 }}>
        {title}
      </p>
      <p style={{ color: "var(--color-label-quaternary)", fontSize: "0.78rem" }}>
        {milestone} 中实现
      </p>
    </div>
  );
}
