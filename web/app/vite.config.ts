// The app's dev server and build (ARCHITECTURE, Build, run, deploy: Vite serves the UI with
// hot reload and proxies API calls to a running server, or runs the in-browser host with no
// server). CAIRN_SERVER is the server's origin to proxy to; CAIRN_APP_PORT the port to serve
// on (the browser tests pick free ones).
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import { defineConfig, type ProxyOptions } from "vite";

/**
 * The paths the server keeps (crates/api/src/endpoints.rs, `PREFIX` and `RESERVED`, and the
 * health check); every other path is the app's, which Vite answers with the page.
 */
const SERVER_PATHS = ["/api/", "/.well-known/", "/healthz"];

const server = process.env["CAIRN_SERVER"];
const proxy: Record<string, ProxyOptions> = {};
if (server !== undefined) {
  for (const path of SERVER_PATHS) {
    proxy[path] = { target: server, changeOrigin: false };
  }
}

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  plugins: [react()],
  logLevel: "warn",
  // `mise run build:web` writes here, and the binary embeds it (crates/cairn/src/assets.rs);
  // beside it in dist/ sit the browser tests' reports, which are never embedded.
  build: { outDir: "dist/build", emptyOutDir: true },
  // The dev server bundles dependencies it finds by crawling from index.html, which never
  // follows `new Worker(new URL(...))`. A dependency only a worker imports (ELK's worker
  // build, canvas/layout-worker.ts) was found when the first page started that worker: Vite
  // bundled again and reloaded every open page, and a page still loading got 504s for the
  // modules it had asked for, so the first browser tests on a cold cache failed at random.
  // Crawling the workers too finds everything before the first page loads.
  optimizeDeps: { entries: ["index.html", "src/**/*-worker.ts"] },
  server: {
    host: "127.0.0.1",
    port: Number(process.env["CAIRN_APP_PORT"] ?? "5173"),
    strictPort: true,
    proxy,
  },
});
