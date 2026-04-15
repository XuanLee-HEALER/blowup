import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  // Pre-bundle heavy dependencies so first page load doesn't block on transforms
  optimizeDeps: {
    include: ["react", "react-dom", "react-router-dom", "d3", "react-markdown", "remark-gfm", "dompurify"],
  },
  build: {
    target: "chrome105",
    minify: !process.env.TAURI_DEBUG,
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        splash: resolve(__dirname, "splash.html"),
        player: resolve(__dirname, "player.html"),
        waveform: resolve(__dirname, "waveform.html"),
        "subtitle-viewer": resolve(__dirname, "subtitle-viewer.html"),
      },
    },
  },
});
