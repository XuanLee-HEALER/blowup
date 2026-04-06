// src/components/ui/TextInput.tsx
import React from "react";

interface TextInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  leadingIcon?: React.ReactNode;
}

export function TextInput({ leadingIcon, style, ...props }: TextInputProps) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "0.5rem",
        background: "var(--color-bg-control)",
        border: "1px solid var(--color-separator)",
        borderRadius: "8px",
        padding: "0 0.75rem",
        height: "34px",
        ...style,
      }}
    >
      {leadingIcon && (
        <span style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>
          {leadingIcon}
        </span>
      )}
      <input
        {...props}
        style={{
          background: "none",
          border: "none",
          outline: "none",
          color: "var(--color-label-primary)",
          fontSize: "0.85rem",
          fontFamily: "inherit",
          flex: 1,
          width: 0,
        }}
      />
    </div>
  );
}
