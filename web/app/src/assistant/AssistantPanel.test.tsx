// What the assistant panel shows of a turn (I5, I7): the conversation shows as the host kept it.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { Transcript } from "./AssistantPanel.tsx";

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
