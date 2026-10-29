// What sits over the graph: the Signal button at the top right (the lens, Key; a route's canvas has no signals, so its Key button takes the slot) and Reveal at
// the bottom right. The rest of the controls are the view's own (screens/ViewControls.tsx) and the
// zoom buttons (GraphCanvas). Quiet by rule: one control each.
import { Panel } from "@xyflow/react";
import { useId } from "react";

import { Menu } from "../screens/Menu.tsx";
import { LENS_MEANING, LENS_WORDS } from "./lens.ts";
import { DateSample, LineSample } from "./LineSample.tsx";
import { LENSES, LINE_KINDS, type Lens } from "./model.ts";
import { DATE_LINE_KEY, LINE_KIND_WORDS } from "./words.ts";
import { plural } from "../ui/words.ts";
import { Tip } from "../ui/Tip.tsx";

/** The kinds of line and the date line, drawn as the canvas draws them, for View, Key. */
function Key() {
  return (
    <ul className="canvas-key" data-testid="canvas-key">
      {LINE_KINDS.map((kind) => (
        <li key={kind}>
          <LineSample kind={kind} />
          <span>{`${LINE_KIND_WORDS[kind].label}: ${LINE_KIND_WORDS[kind].key}`}</span>
        </li>
      ))}
      <li>
        <DateSample />
        <span>{DATE_LINE_KEY}</span>
      </li>
    </ul>
  );
}

/** A route's canvas button at the top right, where a journey's Signal button is: a route has no signals to show, so its menu is the key to the lines alone. */
export function KeyMenu() {
  return (
    <Menu label="Key to the lines" tip="What the lines and their ends mean" testId="key-menu" role="dialog" align="end" className="canvas-signal" trigger={"Key \u25be"}>
      {() => (
        <div className="stack" data-testid="key-panel">
          <Key />
        </div>
      )}
    </Menu>
  );
}

/** The one canvas button for how the graph is drawn, at the top right: the signal each card shows, and the other ways the graph is drawn (the key to the lines). */
export function SignalMenu({ lens, onLens }: { lens: Lens | undefined; onLens: (lens: Lens | undefined) => void }) {
  const group = useId();
  const options: [Lens | undefined, string][] = [[undefined, "Off"], ...LENSES.map((each) => [each, LENS_WORDS[each]] as [Lens, string])];
  return (
    <Menu
      label={`Signal: ${lens === undefined ? "Off" : LENS_WORDS[lens]}`}
      tip="Which priority signal each card shows: rank"
      testId="view-menu"
      role="dialog"
      align="end"
      className={lens === undefined ? "canvas-signal" : "canvas-signal menu-pressed"}
      trigger={`Signal: ${lens === undefined ? "Off" : LENS_WORDS[lens]} \u25be`}
    >
      {() => (
        <div className="stack" data-testid="view-panel">
          <div className="opts" role="radiogroup" aria-label="Signal" data-testid="signals-choice">
            {options.map(([each, words]) => (
              <label key={words} className="opt">
                <input type="radio" name={group} checked={lens === each} onChange={() => { onLens(each); }} />
                <span>{words}</span>
              </label>
            ))}
          </div>
          {lens === undefined ? null : <span className="muted small">{LENS_MEANING[lens]}</span>}
          <hr className="menu-rule" />
          <details>
            <summary>Key to the lines</summary>
            <Key />
          </details>
        </div>
      )}
    </Menu>
  );
}

/**
 * Reveal, at the bottom right while the selected node's trace has nodes without a card of their own:
 * it draws them (with the rest of the trace), and Hide takes them off again. `linked` is the count
 * Reveal was offered for; it stays on screen while they are drawn.
 */
export function RevealBox({ linked, revealed, onReveal, onHide }: { linked: number; revealed: boolean; onReveal: () => void; onHide: () => void }) {
  if (linked === 0 && !revealed) {
    return null;
  }
  return (
    <Panel position="bottom-right" className="canvas-panel canvas-reveal">
      <Tip tip={revealed ? "Draw only what the view shows again" : "Draw the nodes the trace reaches that have no card at this depth"}>
        <button type="button" className="button" data-testid="trace-reveal" onClick={revealed ? onHide : onReveal}>
          {revealed ? "Hide revealed nodes" : `Reveal ${plural(linked, "linked node")}`}
        </button>
      </Tip>
    </Panel>
  );
}
