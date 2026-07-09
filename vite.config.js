import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
// IMPORTANT: Use 127.0.0.1 (not bare "localhost") so WebView2 does not try IPv6 ::1
// while the server only listens on IPv4 — that caused "can't reach this page".
export default defineConfig(async () => ({
  plugins: [sveltekit()],

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Bind IPv4 loopback only; must match tauri.conf.json build.devUrl
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : {
          protocol: "ws",
          host: "127.0.0.1",
          port: 1420,
        },
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
