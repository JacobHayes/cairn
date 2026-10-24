// The assistant step's pane: an example, never the live panel. The practice journey is real, so
// the real panel would send a person's words to the assistant about it; the tour shows what the
// panel looks like and what it answers from the same facts the engine derived, and sends nothing.
import { useMemo, useState } from "react";

import { AssistantDock, journeyTitles, Transcript } from "../assistant/AssistantPanel.tsx";
import { assistantOffered } from "../assistant/model.ts";
import { useJourney, useSession } from "../data/react.ts";
import { AssistantSlot, useAssistantDock } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { useTour } from "./Tour.tsx";
import { EXAMPLE_QUESTION, exampleReply, factsOf } from "./steps.ts";

function Example({ journey }: { journey: string }) {
  const tour = useTour();
  const view = useJourney(journey);
  const replied = tour?.replied === true;
  const entries = useMemo(
    () => (view.status === "ready" && replied ? [{ entry: "said" as const, at: "tour-said", text: EXAMPLE_QUESTION }, { entry: "reply" as const, at: "tour-reply", text: exampleReply(factsOf(view)) }] : []),
    [view, replied],
  );
  return (
    <section className="panel assistant-panel" aria-label="Assistant" data-testid="tour-assistant-example">
      <div className="assistant-scroll inspector-body">
        <p className="muted small">Example only. No message is sent.</p>
        <Transcript entries={entries} titleOf={view.status === "ready" ? journeyTitles(view) : undefined} />
      </div>
      {replied ? null : (
        <form className="stack" onSubmit={(event) => { event.preventDefault(); }}>
          <div className="stack assistant-ask">
            <span>Example question</span>
            <p className="assistant-said" data-testid="tour-assistant-question">{EXAMPLE_QUESTION}</p>
          </div>
          <span className="row">
            <Button primary type="button" disabled={view.status !== "ready"} data-testid="tour-assistant-show" onClick={() => { tour?.setReplied(true); }}>
              Show example reply
            </Button>
          </span>
        </form>
      )}
    </section>
  );
}

function ExampleDock({ journey }: { journey: string }) {
  const session = useSession();
  const [open, setOpen] = useState(false);
  const dock = useMemo(() => ({ open: () => { setOpen(true); }, close: () => { setOpen(false); } }), []);
  useAssistantDock(assistantOffered(session) ? dock : undefined);
  return open ? (
    <AssistantSlot>
      <Example journey={journey} />
    </AssistantSlot>
  ) : null;
}

/** The journey header's assistant: the example on the tour's practice journey, the live panel on every other. */
export function JourneyAssistant({ journey, ...rest }: { journey: string } & Omit<Parameters<typeof AssistantDock>[0], "target">) {
  const tour = useTour();
  // Until the record is read it is not known which journey is the practice one, and the live panel must never open on it.
  if (tour?.reading === true) {
    return null;
  }
  return tour?.practice === journey ? <ExampleDock journey={journey} /> : <AssistantDock target={{ journey }} {...rest} />;
}
