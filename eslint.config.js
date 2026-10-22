// PRACTICES, Code conventions (TypeScript): eslint with the type-aware rules, functions of
// at most 70 lines, and no `any` outside generated code, which is not linted at all. This
// file configures the linter and is not part of the TypeScript project, so it is not linted.
import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

// The browser tests wait on events, never on time (PRACTICES, Shell: conformance and integration).
// A read that does not retry (a Locator's or a Page's `count`, `getAttribute`, `boundingBox`...)
// gives the page's state at one moment, which may be about to change; it is allowed only where
// the page has settled first (`whenSettled` in e2e/shell.ts) or where it is retried (`expect.poll`,
// `toPass`). The receiver's type tells a Playwright read from the same name on a DOM element. An
// `evaluate` whose answer is dropped is an action the test takes on the page, not a read.
const SNAPSHOT_READS = new Set([
  "all",
  "allInnerTexts",
  "allTextContents",
  "boundingBox",
  "count",
  "evaluate",
  "evaluateAll",
  "getAttribute",
  "innerHTML",
  "innerText",
  "inputValue",
  "isChecked",
  "isDisabled",
  "isEditable",
  "isEnabled",
  "isHidden",
  "isVisible",
  "textContent",
]);
const PLAYWRIGHT_HANDLES = new Set(["Locator", "Page", "Frame", "ElementHandle"]);
/** Whether `call` is `whenSettled(...)`, `expect.poll(...)` or `....toPass(...)`. */
const retriesOrSettles = (call) =>
  call.type === "CallExpression" &&
  ((call.callee.type === "Identifier" && call.callee.name === "whenSettled") ||
    (call.callee.type === "MemberExpression" &&
      call.callee.property.type === "Identifier" &&
      (call.callee.property.name === "toPass" || (call.callee.property.name === "poll" && call.callee.object.type === "Identifier" && call.callee.object.name === "expect"))));
const settledReads = {
  meta: { type: "problem", messages: { unsettled: "`{{name}}` reads the page at one moment: read it in whenSettled, or assert it with a web-first assertion or expect.poll." } },
  create(context) {
    const services = context.sourceCode.parserServices;
    return {
      CallExpression(node) {
        const callee = node.callee;
        if (callee.type !== "MemberExpression" || callee.property.type !== "Identifier" || !SNAPSHOT_READS.has(callee.property.name)) {
          return;
        }
        const type = services.getTypeAtLocation(callee.object);
        const name = (type.getSymbol() ?? type.aliasSymbol)?.getName();
        const dropped = callee.property.name.startsWith("evaluate") && node.parent.type === "AwaitExpression" && node.parent.parent.type === "ExpressionStatement";
        if (name === undefined || !PLAYWRIGHT_HANDLES.has(name) || dropped || context.sourceCode.getAncestors(node).some(retriesOrSettles)) {
          return;
        }
        context.report({ node, messageId: "unsettled", data: { name: callee.property.name } });
      },
    };
  },
};
const TIME = "Wait for what the page shows (a web-first assertion, or settled in e2e/shell.ts), never for time; a timer the app runs is the test's to run with page.clock.";

export default defineConfig(
  {
    ignores: ["**/generated/**", "**/node_modules/**", "**/dist/**", "target/**", "eslint.config.js"],
  },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  {
    // Every browser spec takes `test` from e2e/test.ts, which checks the invariants every page must hold after each test.
    files: ["web/app/e2e/**/*.ts"],
    ignores: ["web/app/e2e/test.ts"],
    plugins: { e2e: { rules: { "settled-reads": settledReads } } },
    rules: {
      "no-restricted-imports": ["error", { paths: [{ name: "@playwright/test", importNames: ["test"], message: "Import test from ./test.ts: it checks the invariants every page must hold after every test." }] }],
      "e2e/settled-reads": "error",
      "no-restricted-syntax": [
        "error",
        { selector: "CallExpression[callee.property.name='waitForTimeout']", message: TIME },
        { selector: "CallExpression[callee.name=/^(setTimeout|requestAnimationFrame)$/], CallExpression[callee.property.name=/^(setTimeout|requestAnimationFrame)$/]", message: TIME },
        { selector: "Literal[value='networkidle']", message: TIME },
        { selector: "Property[key.name='timeout']", message: `${TIME} The config's timeouts are the only budget.` },
      ],
    },
  },
  {
    // The invariants every page holds after each test are checked on whatever state it is in, settled or not.
    files: ["web/app/e2e/containment.ts", "web/app/e2e/internal-ids.ts"],
    rules: { "e2e/settled-reads": "off" },
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
