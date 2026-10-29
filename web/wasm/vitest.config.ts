// The web-unit step's Node tests of the engine package: Vitest over src/, against the module the
// build bound (dist/bindgen).
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: { include: ["src/**/*.test.ts"], environment: "node", testTimeout: 120_000 },
});
