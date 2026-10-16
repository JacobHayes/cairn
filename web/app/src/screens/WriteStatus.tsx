// What a control shows of its write when it is not rejected: still being resent, or in need of
// the person (no answer came, it was not sent, its subject is gone). The write is the edit
// manager's, so this reads it and sends it Retry or Discard; nothing is held here. A control
// in a transient bar asks for the `line` layout: the same words and actions without the box,
// and a write the rules refused lists every rule it broke.
import { unlandedWords } from "../data/edit-manager.ts";
import { useSession, useSkew } from "../data/react.ts";
import type { WriteView } from "../data/write-machine.ts";
import { Button } from "../ui/kit.tsx";
import { ViolationLines } from "./RejectionView.tsx";

export function WriteStatus({ view, layout = "callout" }: { view: WriteView | undefined; layout?: "callout" | "line" }) {
  const { edits } = useSession();
  const skew = useSkew();
  if (view === undefined) {
    return null;
  }
  if (view.state === "unconfirmed" && view.retrying) {
    return (
      <span className="muted small" data-testid="write-retrying">
        Not confirmed yet; trying again.
      </span>
    );
  }
  const words = unlandedWords(view, skew);
  if (words === undefined) {
    return null;
  }
  const actions = (
    <span className="row">
      {words.retry && !skew ? (
        <Button
          onClick={() => {
            edits.send({ type: "retry", id: view.id });
          }}
        >
          Retry
        </Button>
      ) : null}
      <Button
        onClick={() => {
          edits.send({ type: "discard", id: view.id });
        }}
      >
        Discard
      </Button>
    </span>
  );
  if (layout === "line") {
    return (
      <div className="stack" role="alert" data-testid="write-problem">
        {view.state === "rejected" && view.rejection.rejection === "invalid" ? <ViolationLines violations={view.rejection.violations} /> : <span className="author-problem">{words.message}</span>}
        {actions}
      </div>
    );
  }
  return (
    <div className="callout callout-bad stack" role="alert" data-testid="write-problem">
      <span>{words.message}</span>
      {actions}
    </div>
  );
}
