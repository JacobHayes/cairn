// What sits over the graph (5.8): the detail ladder at the top left, the Signal button beside it (the lens, Key), the Select mode's band, and the status bar along the bottom for what the
// picture cannot show (nodes hidden). Quiet by rule: one control each.
import { Panel } from "@xyflow/react";
import { useId, type ReactNode } from "react";

import { Menu } from "../screens/Menu.tsx";
import { Button, Segmented } from "../ui/kit.tsx";
import { STEP_WORDS } from "./ladder.ts";
import { LENS_MEANING, LENS_WORDS } from "./lens.ts";
import { LENSES, type Lens, type Step } from "./model.ts";

/**
 * C2: the detail ladder (All, Work, Decisions, Stages: general to specific), which replaces the kind checkboxes. The
 * trace bar or the Select band shares its row, and wraps under it when the canvas is narrow.
 */
export function Ladder({ steps, step, onStep, children }: { steps: readonly Step[]; step: Step; onStep: (step: Step) => void; children?: ReactNode }) {
  return (
    <Panel position="top-left" className="canvas-panel canvas-top">
      <Segmented label="Detail" value={step} options={[...steps].reverse().map((each) => ({ value: each, label: STEP_WORDS[each] }))} onChange={onStep} data-testid="ladder" />
      {children}
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

/** The one canvas button for how the graph is drawn, in the ladder's row so nothing floats over a card's tag at the top right: the signal each card shows, and the other ways the graph is drawn (the key to the lines). */
export function SignalMenu({ lens, onLens }: { lens: Lens | undefined; onLens: (lens: Lens | undefined) => void }) {
  const group = useId();
  const options: [Lens | undefined, string][] = [[undefined, "Off"], ...LENSES.map((each) => [each, LENS_WORDS[each]] as [Lens, string])];
  return (
    <Menu
      label={`Signal: ${lens === undefined ? "Off" : LENS_WORDS[lens]}`}
      title="Which priority signal each card shows: rank, gravity, unlocks or slack"
      testId="view-menu"
      role="dialog"
      align="start"
      className={lens === undefined ? "canvas-signal" : "canvas-signal canvas-signal-on"}
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
          <hr className="canvas-signal-rule" />
          <details>
            <summary>Key to the lines</summary>
            <Key />
          </details>
        </div>
      )}
    </Menu>
  );
}

/** The Select mode's band (5.9). Editing structure has its own full-width band under the header (authoring/JourneyAuthoring.tsx). */
export function SelectBand({ onDone }: { onDone: () => void }) {
  return (
    <div className="canvas-trace" data-testid="select-band">
      <strong>Selecting</strong>
      <span>shift-click or shift-drag</span>
      <Button onClick={onDone}>Done</Button>
    </div>
  );
}

/**
 * The status bar along the bottom of the canvas: what the picture cannot show, each with its way
 * to show it: the selection's traced nodes that are hidden (Reveal), and the settled not-relevant
 * nodes left out (Show). It is absent when it has nothing to say, and anchored to the canvas's
 * edge, so opening the inspector moves nothing in it.
 */
export function CanvasStatus({ linked, notRelevant, onReveal, onShow }: { linked: number; notRelevant: number; onReveal: (() => void) | undefined; onShow: () => void }) {
  if (linked === 0 && notRelevant === 0) {
    return null;
  }
  return (
    <Panel position="bottom-left" className="canvas-status" data-testid="canvas-status">
      {notRelevant === 0 ? null : (
        <span data-testid="hidden-count">
          <span className="status-text">{notRelevant} not relevant hidden &middot;</span>
          <button type="button" className="link" data-testid="hidden-show" onClick={onShow}>
            Show
          </button>
        </span>
      )}
      {linked === 0 ? null : (
        <span data-testid="linked-hidden">
          <span className="status-text">
            {linked} linked {linked === 1 ? "node" : "nodes"} hidden{onReveal === undefined ? "" : " \u00b7"}
          </span>
          {onReveal === undefined ? null : (
            <button type="button" className="link" data-testid="trace-reveal" onClick={onReveal}>
              Reveal
            </button>
          )}
        </span>
      )}
    </Panel>
  );
}
