// Rung 6's web unit tests of the app's data layer (PRACTICES, The validation ladder): Vitest
// in Node over src/, where each module's tests sit beside it, against a fake host and deriver.
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: { include: ["src/**/*.test.ts"], environment: "node" },
});
