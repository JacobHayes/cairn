// The assistant panel's gating and what it shows of a turn (I5, I7): offered only when the
// host's capabilities say there is an assistant, so the in-browser host never shows it; a
// direct change shows as Cairn's report, linking the nodes it names, and a proposal opens in
// proposal review.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { seeded } from "../authoring/engine.test-support.ts";
import type { AssistantAction, AssistantHost } from "../data/assistant.ts";
import { browserHost } from "../data/browser-host.ts";
import { FakeDeriver, FakeHost } from "../data/fake.test-support.ts";
import type { Host } from "../data/host.ts";
import { Session } from "../data/session.ts";
import { NODE_GONE } from "../ui/gone.ts";
import { Transcript } from "./AssistantPanel.tsx";
import { assistantOffered, type Entry } from "./model.ts";

const anAssistant: AssistantHost = {
  conversation: () => Promise.resolve({ conversation: "cv_one", messages: [] }),
  turn: () => Promise.resolve({ outcome: "failed", error: { status: 0, message: "not in this test" } }),
};

/** Whether the rail offers the assistant over `host`, once the session has started. */
async function offeredOver(host: Host): Promise<boolean> {
  return assistantOffered(await Session.start({ host, deriver: new FakeDeriver() }));
}

function serverHost({ offers, assistant }: { offers: boolean; assistant: AssistantHost | undefined }): FakeHost {
  const host = new FakeHost();
  host.offersAssistant = offers;
  host.assistant = assistant;
  return host;
}

describe("the assistant panel's gating (I5, capabilities)", () => {
  it("is offered on a host whose capabilities say it has an assistant", async () => {
    expect(await offeredOver(serverHost({ offers: true, assistant: anAssistant }))).toBe(true);
  });

  it("is absent when the capabilities offer no assistant, whatever the host could reach", async () => {
    for (const assistant of [anAssistant, undefined]) {
      expect(await offeredOver(serverHost({ offers: false, assistant }))).toBe(false);
    }
  });

  it("is never shown on the in-browser host", async () => {
    const host = browserHost(await seeded());
    expect((await host.capabilities()).assistant).toBe(false);
    expect(host.assistant).toBeUndefined();
    expect(await offeredOver(host)).toBe(false);
  });
});

const count = (markup: string, needle: string) => markup.split(needle).length - 1;

const AT = "2026-10-07T10:00:05Z";

function rendered(entries: Entry[]): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <Transcript entries={entries} journey="j_one" titleOf={(key) => (key === "n_kickoff" ? "Kickoff" : undefined)} />
    </MemoryRouter>,
  );
}

/** `actions` as this tab shows a turn it sent. */
const shown = (actions: AssistantAction[]) => rendered(actions.map((action): Entry => ({ entry: "action", at: AT, action })));

describe("a turn's writes in the panel (I5, D7)", () => {
  it("shows a direct change as it lands the same as the conversation keeps it", () => {
    const report = "Added a note, marked done: `j_one/n_kickoff`. Newly stale: `j_one/n_kickoff`.";
    const applied: AssistantAction = { outcome: "applied", report, receipt: { patch_id: "p_one", domain: { journey: "j_one" }, revision: 3 }, nodes: ["n_kickoff"] };
    const live = rendered([{ entry: "action", at: AT, action: applied }]);
    const kept = rendered([{ entry: "report", at: AT, text: report }]);
    expect(live.replace("assistant-applied", "assistant-report")).toBe(kept);
  });

  it("links a node of this journey by its title, says a removed one in words, and links another journey's", () => {
    const markup = rendered([{ entry: "report", at: AT, text: "Marked done: `j_one/n_kickoff`, `j_one/n_gone`, `j_two/n_kickoff`." }]);
    expect(markup).toContain('href="/journeys/j_one/nodes/n_kickoff"');
    expect(markup).toContain(">Kickoff</a>");
    expect(markup).not.toContain("n_gone");
    expect(markup).toContain(NODE_GONE);
    expect(markup).toContain('href="/journeys/j_two/nodes/n_kickoff"');
    expect(count(markup, ">Kickoff</a>")).toBe(1);
  });

  it("reports a write answered from an earlier receipt as already applied, not as a change", () => {
    const markup = shown([
      { outcome: "already_applied", tool: "snooze", receipt: { patch_id: "p_one", domain: { journey: "j_one" }, revision: 3 } },
    ]);
    expect(markup).toContain('data-testid="assistant-already-applied"');
    expect(markup).not.toContain('data-testid="assistant-applied"');
  });

  it("opens a drafted proposal in proposal review, whatever made it a proposal", () => {
    const reasons: AssistantAction[] = [
      { outcome: "proposed", tool: "apply_patch", proposal: "pr_one", destination: { journey: "j_one" }, because: { reason: "structural" } },
      { outcome: "proposed", tool: "assign", proposal: "pr_two", destination: { route: "r_two" }, because: { reason: "structural" } },
      { outcome: "proposed", tool: "create_proposal", proposal: "pr_three", destination: "deployment", because: { reason: "asked" } },
    ];
    const markup = shown(reasons);
    for (const href of ["/journeys/j_one/proposals/pr_one", "/routes/r_two/proposals/pr_two", "/identities/proposals/pr_three"]) {
      expect(markup).toContain(`href="${href}"`);
    }
    expect(count(markup, 'data-testid="review-proposal"')).toBe(reasons.length);
  });
});
