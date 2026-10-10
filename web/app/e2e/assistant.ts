// What the assistant panel's browser tests and proof share (brief 5.8): the demo server's
// scripted model, set to answer the next turn with given tool calls and words
// (crates/cairn/src/demo.rs), and the panel opened and a message sent the way a person does.
import { expect, type Locator, type Page } from "@playwright/test";

/** One step of the model's script: words, tool calls, or both, optionally slow. */
export interface ScriptStep {
  say?: string;
  calls?: { name: string; arguments: unknown }[];
  after_ms?: number;
}

/** Sets what the scripted model answers next, replacing what an earlier script left unplayed. */
export async function script(page: Page, steps: ScriptStep[]): Promise<void> {
  const response = await page.request.put("/demo/assistant/script", { data: steps });
  expect(response.ok(), await response.text()).toBe(true);
}

/** A fresh patch id (H5). */
export function patchId(): string {
  return `p_${crypto.randomUUID().replaceAll("-", "")}`;
}

/** Opens the panel from the screen's header, unless it is open already. */
export async function openPanel(page: Page): Promise<Locator> {
  const panel = page.getByTestId("assistant-panel");
  if (!(await panel.isVisible())) {
    await page.getByTestId("assistant-toggle").click();
  }
  await expect(panel).toBeVisible();
  await expect(panel.getByTestId("assistant-send")).toBeDisabled();
  return panel;
}

/** Sends `message` and waits for the turn's answer. */
export async function ask(page: Page, message: string): Promise<Locator> {
  const panel = page.getByTestId("assistant-panel");
  await panel.getByTestId("assistant-message").fill(message);
  await panel.getByTestId("assistant-send").click();
  await expect(panel).toHaveAttribute("data-status", "idle");
  await expect(panel.getByTestId("assistant-failed")).toHaveCount(0);
  return panel;
}
