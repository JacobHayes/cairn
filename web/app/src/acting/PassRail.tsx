// C11: the pass rail, in the inspector column beside the cards: what is up next, what was
// passed, and what was finished since the pass began, each a link that opens that node in the
// inspector (with a way back to the pass). A long queue shows its first few and says how many
// more there are.
import { Link, useLocation } from "react-router";

import { titleOf, type Ready } from "../detail/model.ts";
import { nodePath, screenPath } from "../detail/parts.tsx";
import { Button } from "../ui/kit.tsx";
import { doneThisPass, type Pass } from "./pass.ts";

/** How many nodes a rail group names before it says how many more. */
const SHOWN = 5;

/** Whether the journey has finished `key`: done, reached or skipped. */
function finished(view: Ready, key: string): boolean {
  const state = view.derived.nodes[key]?.display_state;
  return state === "done" || state === "skipped";
}

function Group({ view, title, keys, testId }: { view: Ready; title: string; keys: readonly string[]; testId: string }) {
  const { pathname, search } = useLocation();
  if (keys.length === 0) {
    return null;
  }
  return (
    <section className="stack pass-group" aria-label={title} data-testid={testId}>
      <span className="muted small">
        {title} <span className="mono">{keys.length}</span>
      </span>
      <ul className="plain-list stack">
        {keys.slice(0, SHOWN).map((key) => (
          <li key={key} data-node={key}>
            <Link to={{ pathname: nodePath(screenPath(pathname), key), search }}>{titleOf(view, key)}</Link>
          </li>
        ))}
        {keys.length > SHOWN ? <li className="muted small">and {keys.length - SHOWN} more</li> : null}
      </ul>
    </section>
  );
}

/** C11: the pass at a glance. `order` is the cards in the pass's order, the passed at its end. */
export function PassRail({ view, order, pass, onNewPass }: { view: Ready; order: readonly string[]; pass: Pass; onNewPass: () => void }) {
  const passed = order.filter((key) => pass.passed.includes(key));
  const waiting = order.filter((key) => !pass.passed.includes(key));
  return (
    <section className="detail-panel panel stack pass-rail" aria-label="This pass" data-testid="pass-rail">
      <Group view={view} title="Up next" keys={waiting.slice(1)} testId="up-next" />
      <Group view={view} title="Passed" keys={passed} testId="passed" />
      <Group view={view} title="Done in this pass" keys={doneThisPass(pass, (key) => finished(view, key))} testId="done-this-pass" />
      <Button onClick={onNewPass} data-testid="new-pass">
        Start a new pass
      </Button>
    </section>
  );
}
