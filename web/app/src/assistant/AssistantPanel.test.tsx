// The assistant panel's gating and what it shows of a turn (I5, I7): offered only when the
// host's capabilities say there is an assistant; the conversation shows as the host kept it.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { AssistantHost } from "../data/assistant.ts";
import { FakeDeriver, FakeHost } from "../data/fake.test-support.ts";
import type { Host } from "../data/host.ts";
import { Session } from "../data/session.ts";
import { Transcript } from "./AssistantPanel.tsx";
import { assistantOffered } from "./model.ts";

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
});

describe("the conversation in the panel (I5)", () => {
  it("shows each line as its author's: the user's words, Cairn's receipt, the reply", () => {
    const markup = renderToStaticMarkup(
      <Transcript
        entries={[
          { author: "user", at: "t1", content: "Kickoff happened." },
          { author: "cairn", at: "t2", content: "Applied a change to this journey." },
          { author: "assistant", at: "t2", content: "Done." },
        ]}
      />,
    );
    expect(markup.indexOf("assistant-said")).toBeLessThan(markup.indexOf("assistant-report"));
    expect(markup.indexOf("assistant-report")).toBeLessThan(markup.indexOf("assistant-reply"));
    expect(markup).toContain("Applied a change to this journey.");
  });
});
