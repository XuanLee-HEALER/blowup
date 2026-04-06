// src/components/ui/Button.tsx
import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "ghost";
}

export function Button({ variant = "ghost", style, children, ...props }: ButtonProps) {
  return (
    <button
      {...props}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "0.35rem",
        padding: "0.35rem 0.75rem",
        borderRadius: "6px",
        border: "none",
        cursor: props.disabled ? "default" : "pointer",
        fontFamily: "inherit",
        fontSize: "0.82rem",
        background:
          variant === "primary"
            ? "var(--color-accent)"
            : "var(--color-bg-control)",
        color:
          variant === "primary"
            ? "#000"
            : "var(--color-label-secondary)",
        opacity: props.disabled ? 0.25 : 1,
        pointerEvents: props.disabled ? "none" : "auto",
        ...style,
      }}
    >
      {children}
    </button>
  );
}
