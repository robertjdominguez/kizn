import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Browser loop: Vite on 127.0.0.1, proxying /rpc to the dev bridge. Both stay
// on localhost; `tailscale serve` (just dev tailnet) is the only way in from
// outside, hence `.ts.net` in allowedHosts. The Tauri window loads the same
// server in `just app` and talks IPC instead.
const port = Number(process.env.KIZN_VITE_PORT ?? 1420);
const bridge = `http://127.0.0.1:${process.env.KIZN_BRIDGE_PORT ?? 7878}`;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port,
    strictPort: true,
    allowedHosts: [".ts.net"],
    proxy: { "/rpc": bridge, "/events": bridge },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // The oldest webview kizn runs in (WebKitGTK / WKWebView / WebView2).
    target: ["safari16", "chrome111"],
  },
});
