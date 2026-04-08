// Shared inline style constants to reduce duplication across components.

export const MODAL_OVERLAY: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(0,0,0,0.5)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  zIndex: 100,
};

export const MODAL_CARD: React.CSSProperties = {
  background: "var(--color-bg-primary)",
  borderRadius: 10,
  padding: "1.5rem",
  border: "1px solid var(--color-separator)",
};

export const INPUT: React.CSSProperties = {
  width: "100%",
  padding: "0.5rem",
  background: "var(--color-bg-elevated)",
  border: "1px solid var(--color-separator)",
  borderRadius: 6,
  color: "var(--color-label-primary)",
  fontSize: "0.82rem",
  fontFamily: "inherit",
  boxSizing: "border-box",
};

export const BTN_SECONDARY: React.CSSProperties = {
  background: "none",
  border: "1px solid var(--color-separator)",
  borderRadius: 6,
  padding: "0.35rem 0.75rem",
  color: "var(--color-label-secondary)",
  cursor: "pointer",
  fontSize: "0.78rem",
  fontFamily: "inherit",
};

export const BTN_ACCENT: React.CSSProperties = {
  background: "var(--color-accent)",
  border: "none",
  borderRadius: 6,
  padding: "0.35rem 0.75rem",
  color: "#000",
  cursor: "pointer",
  fontSize: "0.78rem",
  fontFamily: "inherit",
  fontWeight: 600,
};

export const LABEL: React.CSSProperties = {
  fontSize: "0.72rem",
  color: "var(--color-label-tertiary)",
  display: "block",
  marginBottom: "0.3rem",
};

export const SECTION_HEADER: React.CSSProperties = {
  fontSize: "0.68rem",
  color: "var(--color-label-quaternary)",
  textTransform: "uppercase",
  letterSpacing: "0.06em",
  margin: 0,
  fontWeight: 600,
};
