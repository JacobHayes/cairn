// The web-unit step's tests of the app's data layer (PRACTICES, The validation ladder): Vitest
// in Node over src/, where each module's tests sit beside it, against a fake host and deriver;
// a component renders to static markup. The tests that need the fixtures (src/authoring/engine.test-support.ts)
// each start a `cairn demo` from the binary the run builds (src/demo.global-setup.ts).
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: { include: ["src/**/*.test.ts", "src/**/*.test.tsx"], environment: "node", globalSetup: ["src/demo.global-setup.ts"], setupFiles: ["src/canvas/measure.test-support.ts"] },
});
