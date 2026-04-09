// src/components/ui/Chip.tsx
interface ChipProps {
  label: string;
  active?: boolean;
  onRemove?: () => void;
  onClick?: () => void;
}

export function Chip({ label, active, onRemove, onClick }: ChipProps) {
  return (
    <button
      onClick={onClick}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "0.3rem",
        background: active ? "var(--color-accent-soft)" : "var(--color-bg-control)",
        border: `1px solid ${active ? "var(--color-accent)" : "var(--color-separator)"}`,
        borderRadius: "100px",
        padding: "0.18rem 0.6rem",
        fontSize: "0.72rem",
        color: active ? "var(--color-accent)" : "var(--color-label-secondary)",
        cursor: "pointer",
        fontFamily: "inherit",
      }}
    >
      {label}
      {onRemove && (
        <span
          onClick={(e) => { e.stopPropagation(); onRemove(); }}
          style={{ opacity: 0.6, fontSize: "0.65rem" }}
        >
          ✕
        </span>
      )}
    </button>
  );
}
