// Rung 6's Node tests of the browser host (brief 4.5): Vitest over src/, against the module
// the build bound (dist/bindgen) and the answers the server walk wrote (dist/cases). The
// agreement cases run whole groups through the wasm32 module, so a test may take a while.
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: { include: ["src/**/*.test.ts"], environment: "node", testTimeout: 120_000 },
});
