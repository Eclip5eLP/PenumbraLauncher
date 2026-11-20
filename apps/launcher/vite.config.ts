import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri expects a fixed dev port and no screen clearing (so Rust errors stay visible).
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    copyPublicDir: false, // public/ only holds dev mock media
    target: "chrome120", // WebView2 is evergreen Chromium
    cssMinify: true,
    reportCompressedSize: true,
  },
});
