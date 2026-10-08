// The assistant panel's gating and what it shows of a turn (I5, I7): offered only when the
// host's capabilities say there is an assistant, so the in-browser host never shows it; a
// direct change links to the nodes it wrote and what it caused, and a proposal opens in
// proposal review.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { seeded } from "../authoring/engine.test-support.ts";
import type { AssistantAction, AssistantHost } from "../data/assistant.ts";
import { browserHost } from "../data/browser-host.ts";
import { FakeDeriver, FakeHost, manualTimers } from "../data/fake.test-support.ts";
import type { Host } from "../data/host.ts";
import { SessionContext } from "../data/react.ts";
import { Session } from "../data/session.ts";
import { AssistantDock, Transcript } from "./AssistantPanel.tsx";

const anAssistant: AssistantHost = {
  conversation: () => Promise.resolve({ conversation: "cv_one", messages: [] }),
  turn: () => Promise.resolve({ outcome: "failed", error: { status: 0, message: "not in this test" } }),
};

/** The dock on a journey's screen over `host`, rendered once the session has started. */
async function dockOver(host: Host): Promise<string> {
  const session = await Session.start({ host, deriver: new FakeDeriver(), timers: manualTimers().timers });
  return renderToStaticMarkup(
    <SessionContext value={session}>
      <MemoryRouter>
        <AssistantDock target={{ journey: "j_vendor_eval" }} />
      </MemoryRouter>
    </SessionContext>,
  );
}

function serverHost({ offers, assistant }: { offers: boolean; assistant: AssistantHost | undefined }): FakeHost {
  const host = new FakeHost();
  host.offersAssistant = offers;
  host.assistant = assistant;
  return host;
}

const toggle = 'data-testid="assistant-toggle"';

describe("the assistant panel's gating (I5, capabilities)", () => {
  it("is offered on a host whose capabilities say it has an assistant", async () => {
    expect(await dockOver(serverHost({ offers: true, assistant: anAssistant }))).toContain(toggle);
  });

  it("is absent when the capabilities offer no assistant, whatever the host could reach", async () => {
    for (const assistant of [anAssistant, undefined]) {
      expect(await dockOver(serverHost({ offers: false, assistant }))).not.toContain(toggle);
    }
  });

  it("is never shown on the in-browser host", async () => {
    const host = browserHost(await seeded());
    expect((await host.capabilities()).assistant).toBe(false);
    expect(host.assistant).toBeUndefined();
    expect(await dockOver(host)).toBe("");
  });
});

const count = (markup: string, needle: string) => markup.split(needle).length - 1;

function shown(actions: AssistantAction[]): string {
  const entries = actions.map((action) => ({ entry: "action" as const, at: "2026-10-07T10:00:05Z", action }));
  return renderToStaticMarkup(
    <MemoryRouter>
      <Transcript entries={entries} titleOf={(key) => (key === "n_kickoff" ? "Kickoff" : undefined)} />
    </MemoryRouter>,
  );
}

describe("a turn's writes in the panel (I5, D7)", () => {
  it("links a direct change to each node it wrote and each node it made stale, in that journey", () => {
    const markup = shown([
      {
        outcome: "applied",
        tool: "transition_node",
        receipt: { patch_id: "p_one", domain: { journey: "j_one" }, content_hash: "h", revision: 3 },
        nodes: ["n_kickoff", "n_plan"],
        consequences: { j_one: { stale: [{ node: "n_report", reasons: [] }] } },
      },
    ]);
    for (const node of ["n_kickoff", "n_plan", "n_report"]) {
      expect(markup).toContain(`href="/journeys/j_one/nodes/${node}"`);
    }
    expect(count(markup, 'data-testid="assistant-node"')).toBe(3);
    expect(markup).toContain(">Kickoff</a>");
    expect(markup).toContain('data-kind="stale"');
  });

  it("opens a drafted proposal in proposal review, whatever made it a proposal", () => {
    const reasons: AssistantAction[] = [
      { outcome: "proposed", tool: "apply_patch", proposal: "pr_one", because: { reason: "structural" } },
      { outcome: "proposed", tool: "assign", proposal: "pr_two", because: { reason: "too_many_nodes", count: 12 } },
      { outcome: "proposed", tool: "create_proposal", proposal: "pr_three", because: { reason: "asked" } },
    ];
    const markup = shown(reasons);
    for (const id of ["pr_one", "pr_two", "pr_three"]) {
      expect(markup).toContain(`href="/proposals/${id}"`);
    }
    expect(count(markup, 'data-testid="review-proposal"')).toBe(reasons.length);
  });
});
