// src/components/ui/NavItem.tsx
interface NavItemProps {
  icon: string;
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

export function NavItem({ icon, label, active, disabled, onClick }: NavItemProps) {
  return (
    <button
      onClick={disabled ? undefined : onClick}
      style={{
        display: "flex",
        alignItems: "center",
        gap: "0.55rem",
        width: "100%",
        padding: "0.42rem 0.75rem",
        borderRadius: "6px",
        border: "none",
        cursor: disabled ? "default" : "pointer",
        fontSize: "0.82rem",
        fontFamily: "inherit",
        background: active ? "var(--color-bg-elevated)" : "transparent",
        color: disabled
          ? "var(--color-label-quaternary)"
          : active
          ? "var(--color-label-primary)"
          : "var(--color-label-secondary)",
        fontWeight: active ? 500 : 400,
        pointerEvents: disabled ? "none" : "auto",
        opacity: disabled ? 0.25 : 1,
        textAlign: "left",
      }}
    >
      <span style={{ width: 15, textAlign: "center", fontSize: "0.82rem" }}>
        {icon}
      </span>
      {label}
    </button>
  );
}
