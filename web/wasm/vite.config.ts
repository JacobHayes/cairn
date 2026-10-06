// The browser tests' page server (playwright.config.ts starts it): Vite over this package,
// so the harness page, the loader, the derive worker, the module in dist/bindgen/, and the
// cases in dist/cases/ are all served from one origin, on the port CAIRN_E2E_PORT names.
import { fileURLToPath } from "node:url";

import { defineConfig } from "vite";

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  logLevel: "warn",
  server: { host: "127.0.0.1", port: Number(process.env["CAIRN_E2E_PORT"]), strictPort: true },
});
