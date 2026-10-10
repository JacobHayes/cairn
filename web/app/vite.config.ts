// The app's dev server and build (ARCHITECTURE, Build, run, deploy: Vite serves the UI with
// hot reload and proxies API calls to a running server, or runs the in-browser host with no
// server). CAIRN_SERVER is the server's origin to proxy to; CAIRN_APP_PORT the port to serve
// on (the browser tests pick free ones).
//
// The build fixes the host the app runs on (ARCHITECTURE, Web UI: in-browser host): the dev
// server runs the server host when it proxies to one and the in-browser host when it does
// not; `vite build` makes the build the binary embeds, on the server host, and `vite build
// --mode demo` the static demo site, on the in-browser host.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin, type ProxyOptions } from "vite";

/**
 * The design language's tokens, base styles and fonts, imported from their one source with no
 * copies (DESIGN.md): `import "@design/tokens.css"`.
 */
const DESIGN = fileURLToPath(new URL("../../design", import.meta.url));

/**
 * The module's bindings are checked in (web/wasm/generated/), but its `.wasm` is only where
 * `build:wasm` wrote both, and the bindings' own fallback names the `.wasm` beside them
 * (`new URL("cairn_wasm_bg.wasm", import.meta.url)`). Bundling the bindings from there lets
 * Vite resolve that URL, so it warns of nothing and the page's `?url` import (boot.ts) is the
 * same fingerprinted file.
 */
const BINDGEN = fileURLToPath(new URL("../wasm/dist/bindgen", import.meta.url));

/**
 * The fonts are SIL OFL 1.1, which asks that the license travel with them: the build carries
 * it beside the page, at `/fonts-LICENSE.txt`.
 */
function fontLicense(): Plugin {
  return {
    name: "cairn:font-license",
    generateBundle() {
      this.emitFile({ type: "asset", fileName: "fonts-LICENSE.txt", source: readFileSync(`${DESIGN}/fonts/LICENSE.txt`, "utf8") });
    },
  };
}

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

export default defineConfig(({ command, mode }) => ({
  define: {
    __CAIRN_HOST__: JSON.stringify(command === "serve" ? (server === undefined ? "browser" : "server") : mode === "demo" ? "browser" : "server"),
  },
  root: fileURLToPath(new URL(".", import.meta.url)),
  plugins: [react(), fontLicense()],
  resolve: { alias: [{ find: "@design", replacement: DESIGN }, { find: /^.*\/generated\/cairn_wasm\.js$/, replacement: `${BINDGEN}/cairn_wasm.js` }] },
  logLevel: "warn",
  // `mise run build:web` writes dist/build/, which the binary embeds (crates/cairn/src/assets.rs),
  // and `mise run build:demo` dist/demo/; beside them sit the browser tests' reports.
  build: {
    outDir: mode === "demo" ? "dist/demo" : "dist/build",
    emptyOutDir: true,
    // React and the router are most of what every page loads and change only with an upgrade, so
    // they are one chunk of their own (cached across the app's releases); the screens and the
    // graph (main.tsx, Projections.tsx) are loaded when first opened.
    rolldownOptions: { output: { codeSplitting: { groups: [{ name: "react", test: /node_modules[\\/](react|react-dom|react-router|scheduler)[\\/]/ }] } } },
  },
  // The browser tests run a dev server of each host side by side; each bundles into its own.
  cacheDir: `../../node_modules/.vite/app-${server === undefined ? "demo" : "server"}`,
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
}));
