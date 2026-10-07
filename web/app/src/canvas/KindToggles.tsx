// `KindToggles`: the canvas's switches. Semantic zoom shows or hides each kind independently,
// all shown by default (C2); on a journey, not-relevant and undecided nodes each have a toggle
// (C1), and the heat overlay shows gravity and leverage as numbers (C6).
import type { NodeKind } from "../detail/model.ts";
import { KINDS } from "./model.ts";
import { withKind, type CanvasView } from "./settings.ts";

const KIND_WORDS: Record<NodeKind, string> = {
  group: "Groups",
  decision: "Decisions",
  deliverable: "Deliverables",
  action: "Actions",
  milestone: "Milestones",
};

function Toggle({ label, checked, onChange, testId }: { label: string; checked: boolean; onChange: (checked: boolean) => void; testId: string }) {
  return (
    <label className="toggle" data-testid={testId} data-status={checked ? "on" : "off"}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
      />
      {label}
    </label>
  );
}

/** C1, C2, C6: the kind toggles, and on a journey the relevance and heat toggles. */
export function KindToggles({ view, onChange, journey }: { view: CanvasView; onChange: (view: CanvasView) => void; journey: boolean }) {
  return (
    <div className="toggles" role="group" aria-label="What the canvas shows">
      <span className="row" aria-label="Kinds">
        {KINDS.map((kind) => (
          <Toggle
            key={kind}
            label={KIND_WORDS[kind]}
            checked={view.shown.includes(kind)}
            testId={`show-${kind}`}
            onChange={(checked) => {
              onChange(withKind(view, kind, checked));
            }}
          />
        ))}
      </span>
      {journey ? (
        <span className="row">
          <Toggle
            label="Not relevant"
            checked={view.notRelevant}
            testId="show-not-relevant"
            onChange={(checked) => {
              onChange({ ...view, notRelevant: checked });
            }}
          />
          <Toggle
            label="Undecided"
            checked={view.undecided}
            testId="show-undecided"
            onChange={(checked) => {
              onChange({ ...view, undecided: checked });
            }}
          />
          <Toggle
            label="Heat"
            checked={view.heat}
            testId="show-heat"
            onChange={(checked) => {
              onChange({ ...view, heat: checked });
            }}
          />
        </span>
      ) : null}
    </div>
  );
}
