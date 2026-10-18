// PRACTICES, Code conventions (TypeScript): eslint with the type-aware rules, functions of
// at most 70 lines, and no `any` outside generated code, which is not linted at all. This
// file configures the linter and is not part of the TypeScript project, so it is not linted.
import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(
  {
    ignores: ["**/generated/**", "**/node_modules/**", "**/dist/**", "target/**", "eslint.config.js"],
  },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  {
    // Every browser spec takes `test` from e2e/test.ts, which checks the containment invariant after each test.
    files: ["web/app/e2e/**/*.ts"],
    ignores: ["web/app/e2e/test.ts"],
    rules: {
      "no-restricted-imports": ["error", { paths: [{ name: "@playwright/test", importNames: ["test"], message: "Import test from ./test.ts: it checks the containment invariant after every test." }] }],
    },
  },
  {
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    rules: {
      "max-lines-per-function": ["error", { max: 70 }],
      "@typescript-eslint/no-explicit-any": "error",
    },
  },
);
