// The app's dev server and build (ARCHITECTURE, Build, run, deploy: Vite serves the UI with
// hot reload and proxies API calls to a running server, or runs the in-browser host with no
// server). CAIRN_SERVER is the server's origin to proxy to; CAIRN_APP_PORT the port to serve
// on (the browser tests pick free ones).
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import { defineConfig, type ProxyOptions } from "vite";

/** The API's top-level paths (openapi/), which the UI never uses: it routes by hash. */
const API_PATHS = [
  "/auth",
  "/capabilities",
  "/deployment",
  "/entities",
  "/events",
  "/journeys",
  "/mcp",
  "/proposals",
  "/routes",
  "/search",
  "/users",
];

const server = process.env["CAIRN_SERVER"];
const proxy: Record<string, ProxyOptions> = {};
if (server !== undefined) {
  for (const path of API_PATHS) {
    proxy[path] = { target: server, changeOrigin: false };
  }
}

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  plugins: [react()],
  logLevel: "warn",
  server: {
    host: "127.0.0.1",
    port: Number(process.env["CAIRN_APP_PORT"] ?? "5173"),
    strictPort: true,
    proxy,
  },
});
