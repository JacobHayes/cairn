// What sits over the graph (5.8): the Signal button at the top right (the lens, Key) and Reveal at
// the bottom right. The rest of the controls are the view's own (screens/ViewControls.tsx) and the
// zoom buttons (GraphCanvas). Quiet by rule: one control each.
import { Panel } from "@xyflow/react";
import { useId } from "react";

import { Menu } from "../screens/Menu.tsx";
import { Segmented } from "../ui/kit.tsx";
import { STEP_WORDS } from "./ladder.ts";
import { LENS_MEANING, LENS_WORDS } from "./lens.ts";
import { LENSES, type Lens, type Step } from "./model.ts";

/** C2: a route's Depth (Stages, Decisions, Deliverables, Actions: coarse to fine, as keys 1 to 4). A journey's is the Depth menu of its view controls. */
export function Ladder({ steps, step, onStep }: { steps: readonly Step[]; step: Step; onStep: (step: Step) => void }) {
  return (
    <Panel position="top-left" className="canvas-panel canvas-top">
      <Segmented label="Depth" value={step} options={steps.map((each) => ({ value: each, label: STEP_WORDS[each] }))} onChange={onStep} data-testid="ladder" />
    </Panel>
  );
}

/** The four kinds of line, drawn as the canvas draws them, for View, Key. */
function Key() {
  const samples = [
    { marker: "arrow", dash: undefined, words: "Needs: the line ends in an arrow." },
    { marker: "diamond", dash: "2 3", words: "Applies only if: the line ends in a hollow diamond." },
    { marker: "bar", dash: "2 3", words: "Waits for the stage to open: the line ends in a bar." },
    { marker: "none", dash: "2 3", words: "Dates only: a faint dotted line that does not block." },
  ] as const;
  return (
    <ul className="canvas-key" data-testid="canvas-key">
      {samples.map((sample) => (
        <li key={sample.marker}>
          <svg width="44" height="14" viewBox="0 0 44 14" aria-hidden="true">
            <path d={sample.marker === "none" ? "M2 7 H42" : "M2 7 H32"} stroke="currentColor" fill="none" strokeDasharray={sample.dash} />
            {sample.marker === "arrow" ? <path d="M42 7 L33 2.500 L33 11.500 Z" fill="currentColor" /> : null}
            {sample.marker === "diamond" ? <path d="M42 7 L37.500 2.500 L33 7 L37.500 11.500 Z" fill="var(--color-surface)" stroke="currentColor" /> : null}
            {sample.marker === "bar" ? <path d="M40 2 V12" stroke="currentColor" strokeWidth="2" /> : null}
          </svg>
          <span>{sample.words}</span>
        </li>
      ))}
    </ul>
  );
}

/** The one canvas button for how the graph is drawn, at the top right: the signal each card shows, and the other ways the graph is drawn (the key to the lines). */
export function SignalMenu({ lens, onLens }: { lens: Lens | undefined; onLens: (lens: Lens | undefined) => void }) {
  const group = useId();
  const options: [Lens | undefined, string][] = [[undefined, "Off"], ...LENSES.map((each) => [each, LENS_WORDS[each]] as [Lens, string])];
  return (
    <Menu
      label={`Signal: ${lens === undefined ? "Off" : LENS_WORDS[lens]}`}
      title="Which priority signal each card shows: rank, gravity, unlocks or slack"
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
          <span className="muted small">{lens === undefined ? "Put one number on each card." : LENS_MEANING[lens]}</span>
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
      <button
        type="button"
        className="button"
        title={revealed ? "Draw only what the view shows again" : "Draw the nodes the trace reaches that have no card at this depth"}
        data-testid="trace-reveal"
        onClick={revealed ? onHide : onReveal}
      >
        {revealed ? "Hide revealed nodes" : `Reveal ${String(linked)} linked ${linked === 1 ? "node" : "nodes"}`}
      </button>
    </Panel>
  );
}
