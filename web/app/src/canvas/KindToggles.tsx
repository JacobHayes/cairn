// The canvas's switches. A route's canvas has no ladder, so `KindToggles` shows or hides each
// kind independently (C2), all shown by default. A journey's graph picks out kinds with
// `GraphFilters`, in the toolbar's filter: the kinds left unchecked fade rather than vanish, and
// conditional and not-relevant nodes each have a switch (C1).
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

function Kinds({ view, onChange }: { view: CanvasView; onChange: (view: CanvasView) => void }) {
  return (
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
  );
}

/** C2: the kinds a route's canvas draws. */
export function KindToggles({ view, onChange }: { view: CanvasView; onChange: (view: CanvasView) => void }) {
  return (
    <div className="toggles" role="group" aria-label="What the canvas shows">
      <Kinds view={view} onChange={onChange} />
    </div>
  );
}

/** C1, C2: a journey graph's filter: kinds kept in focus (the rest fade), and which settled nodes are drawn. */
export function GraphFilters({ view, onChange }: { view: CanvasView; onChange: (view: CanvasView) => void }) {
  return (
    <div className="stack" role="group" aria-label="What the graph shows">
      <span className="muted small">Kinds in focus. The others fade.</span>
      <Kinds view={view} onChange={onChange} />
      <Toggle label="Show conditional" checked={view.undecided} testId="show-conditional" onChange={(checked) => { onChange({ ...view, undecided: checked }); }} />
      <Toggle label="Show not relevant" checked={view.notRelevant} testId="show-not-relevant" onChange={(checked) => { onChange({ ...view, notRelevant: checked }); }} />
    </div>
  );
}
