// What a control shows of its write when it is not rejected: still being resent, or in need of
// the person (no answer came, it was not sent, its subject is gone). The write is the edit
// manager's, so this reads it and sends it Retry or Discard; nothing is held here.
import { unlandedWords } from "../data/edit-manager.ts";
import { useSession, useSkew } from "../data/react.ts";
import type { WriteView } from "../data/write-machine.ts";
import { Button } from "../ui/kit.tsx";

export function WriteStatus({ view }: { view: WriteView }) {
  const { edits } = useSession();
  const skew = useSkew();
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
  return (
    <div className="callout callout-bad stack" role="alert" data-testid="write-problem">
      <span>{words.message}</span>
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
    </div>
  );
}
