// Rung 6's web unit tests of the client wrapper (PRACTICES, The validation ladder): Vitest
// in Node over src/, where each module's tests sit beside it.
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: { include: ["src/**/*.test.ts"], environment: "node" },
});
