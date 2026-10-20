// C11: the pass rail, in the inspector column beside the cards: what is up next, what was
// passed, and what was finished since the pass began, each a row that opens that node in the
// inspector (with a way back to the pass). A long queue shows its first few and says how many
// more there are.

import { titleOf, type Ready } from "../detail/model.ts";
import { useNodeAddresses } from "../journeys/address.ts";
import { isFinished } from "../status/state.ts";
import { Button } from "../ui/kit.tsx";
import { NodeItem, NodePanel } from "./NodeItem.tsx";
import { doneThisPass, unlockedBy, type Pass } from "./pass.ts";

/** How many nodes a rail group names before it says how many more. */
const SHOWN = 5;

/** Whether the journey has finished `key`: done, reached or skipped. */
function finished(view: Ready, key: string): boolean {
  const state = view.derived.nodes[key]?.display_state;
  return state !== undefined && isFinished(state);
}

function Group({ view, title, keys, testId, pass }: { view: Ready; title: string; keys: readonly string[]; testId: string; pass?: Pass }) {
  const addresses = useNodeAddresses();
  if (keys.length === 0) {
    return null;
  }
  const by = (key: string) => (pass === undefined ? undefined : unlockedBy(pass, key));
  return (
    <NodePanel title={title} count={keys.length} testId={testId}>
      {keys.slice(0, SHOWN).map((key) => (
        <NodeItem key={key} view={view} node={key} to={addresses.open(key)} fact={by(key) === undefined ? undefined : `Unlocked by ${titleOf(view, by(key) ?? key)}`} />
      ))}
      {keys.length > SHOWN ? <li className="node-row muted small">and {keys.length - SHOWN} more</li> : null}
    </NodePanel>
  );
}

/** C11: the pass at a glance. `order` is the cards in the pass's order, the passed at its end. */
export function PassRail({ view, order, pass, onNewPass }: { view: Ready; order: readonly string[]; pass: Pass; onNewPass: () => void }) {
  const passed = order.filter((key) => pass.passed.includes(key));
  const waiting = order.filter((key) => !pass.passed.includes(key));
  return (
    <section className="stack pass-rail" aria-label="This pass" data-testid="pass-rail">
      <Group view={view} title="Up next" keys={waiting.slice(1)} testId="up-next" pass={pass} />
      <Group view={view} title="Passed" keys={passed} testId="passed" />
      <Group view={view} title="Done in this pass" keys={doneThisPass(pass, (key) => finished(view, key))} testId="done-this-pass" />
      <Button onClick={onNewPass} data-testid="new-pass">
        Start a new pass
      </Button>
    </section>
  );
}
