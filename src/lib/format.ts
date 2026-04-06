export function formatSize(bytes: number | null): string {
  if (!bytes) return "\u2014";
  if (bytes >= 1e9) return (bytes / 1e9).toFixed(2) + " GB";
  if (bytes >= 1e6) return (bytes / 1e6).toFixed(0) + " MB";
  return (bytes / 1e3).toFixed(0) + " KB";
}

export function formatDuration(secs: number | null): string {
  if (!secs) return "\u2014";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  if (h > 0) return `${h}h${m}m${s > 0 ? s + "s" : ""}`;
  return s > 0 ? `${m}m${s}s` : `${m}m`;
}

export function formatBitrate(bps: number | null): string {
  if (!bps) return "\u2014";
  if (bps >= 1e6) return (bps / 1e6).toFixed(1) + " Mbps";
  return (bps / 1e3).toFixed(0) + " kbps";
}

export function formatFrameRate(fr: string | null): string {
  if (!fr) return "\u2014";
  const parts = fr.split("/");
  if (parts.length === 2) {
    const fps = parseFloat(parts[0]) / parseFloat(parts[1]);
    return fps.toFixed(2) + " fps";
  }
  return fr + " fps";
}
