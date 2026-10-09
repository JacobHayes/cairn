// A8: `DateRuleEditor` edits one date rule: `due_by` (finish by) or `not_before` (start no
// sooner than), before or after one or more sources by an offset in days. Sources are
// offered from what a rule may measure from (milestones, date decisions, and when the journey
// started), so the wrong kind of source cannot be entered; several sources are several
// constraints. F4: `StageBoundsEditor` sets a stage's opening and closing milestones, from the
// milestones outside it, and whether each gates and closes.
import { Button, Field } from "../ui/kit.tsx";
import { boundOptions, partsOf, ruleOf, ruleWords, sourceOptions, type Direction, type RuleParts } from "./dates.ts";
import type { DateRule } from "./fields.ts";
import type { Tree } from "./graph.ts";
import { FieldBox, Picker, type FieldNotes } from "./parts.tsx";

export interface DateRuleEditorProps {
  /** `due_by` or `not_before`. */
  which: "due_by" | "not_before";
  value: DateRule | null;
  onChange: (next: DateRule | null) => void;
  tree: Tree;
  /** The node the rule is on, never offered as its own source. */
  node: string | undefined;
}

const WORDS = { due_by: "Finishes by", not_before: "Starts no sooner than" };

function Sources({ parts, set, options }: { parts: RuleParts; set: (next: RuleParts) => void; options: { source: string; label: string }[] }) {
  const unused = options.filter((option) => !parts.sources.includes(option.source));
  const label = (source: string) => options.find((option) => option.source === source)?.label ?? source;
  return (
    <span className="stack">
      {parts.sources.map((source) => (
        <span key={source} className="row" data-testid="rule-source" data-source={source}>
          <span>{label(source)}</span>
          {parts.sources.length > 1 ? (
            <Button aria-label={`Stop measuring from ${label(source)}`} onClick={() => { set({ ...parts, sources: parts.sources.filter((each) => each !== source) }); }}>
              Remove
            </Button>
          ) : null}
        </span>
      ))}
      {unused.length === 0 ? null : (
        <Picker
          aria-label="Also measure from"
          value=""
          none={parts.sources.length === 0 ? "Measure from..." : "Also measure from..."}
          options={unused.map((option) => ({ value: option.source, label: option.label }))}
          onChange={(event) => {
            if (event.target.value !== "") {
              set({ ...parts, sources: [...parts.sources, event.target.value] });
            }
          }}
        />
      )}
    </span>
  );
}

export function DateRuleEditor({ which, value, onChange, tree, node }: DateRuleEditorProps) {
  const options = sourceOptions(tree, node);
  const label = (source: string) => options.find((option) => option.source === source)?.label ?? source;
  if (value === null) {
    return (
      <span className="row" data-testid={`rule-${which}`}>
        <span className="muted small">No rule.</span>
        <Button onClick={() => { onChange(ruleOf({ direction: which === "due_by" ? "before" : "after", sources: [options[0]?.source ?? "journey.created_at"], offset: 0 })); }}>
          Add a rule
        </Button>
      </span>
    );
  }
  const parts = partsOf(value);
  const set = (next: RuleParts) => {
    onChange(ruleOf(next));
  };
  return (
    <div className="stack" data-testid={`rule-${which}`}>
      <span className="muted small" data-testid="rule-words">
        {WORDS[which]} {ruleWords(value, label)}
      </span>
      <span className="row">
        <Field
          type="number"
          min={0}
          aria-label="Offset in days"
          value={String(parts.offset)}
          onChange={(event) => { set({ ...parts, offset: event.target.value === "" ? 0 : Number(event.target.value) }); }}
          style={{ width: "6rem" }}
        />
        <span>days</span>
        <Picker
          aria-label="Before or after"
          value={parts.direction}
          options={[{ value: "before", label: "before" }, { value: "after", label: "after" }]}
          onChange={(event) => { set({ ...parts, direction: event.target.value as Direction }); }}
        />
        <Button aria-label={`Remove the ${which} rule`} onClick={() => { onChange(null); }}>
          Remove the rule
        </Button>
      </span>
      <Sources parts={parts} set={set} options={options} />
    </div>
  );
}

export interface StageBoundsEditorProps {
  opensAt: string;
  closesAt: string;
  gates: boolean;
  closes: boolean;
  onChange: (next: { opens_at?: string; closes_at?: string; gates?: boolean; closes?: boolean }) => void;
  tree: Tree;
  group: string;
  /** What each bound shows beside it (A15). */
  notes?: { opens_at?: FieldNotes; closes_at?: FieldNotes };
}

export function StageBoundsEditor({ opensAt, closesAt, gates, closes, onChange, tree, group, notes = {} }: StageBoundsEditorProps) {
  const options = boundOptions(tree, group).map(({ node, inside }) => ({ value: node.key, label: inside ? `${node.title} (inside this stage)` : node.title }));
  return (
    <div className="stack" data-testid="stage-bounds">
      <FieldBox label="Opens at" place="opens_at" notes={notes.opens_at}>
        <span className="row">
          <Picker aria-label="Opens at" value={opensAt} none="No opening" options={options} onChange={(event) => { onChange({ opens_at: event.target.value }); }} />
          <label className="row">
            <input type="checkbox" checked={gates} disabled={opensAt === ""} onChange={(event) => { onChange({ gates: event.target.checked }); }} />
            its contents wait for it
          </label>
        </span>
      </FieldBox>
      <FieldBox label="Closes at" place="closes_at" notes={notes.closes_at}>
        <span className="row">
          <Picker aria-label="Closes at" value={closesAt} none="No closing" options={options} onChange={(event) => { onChange({ closes_at: event.target.value }); }} />
          <label className="row">
            <input type="checkbox" checked={closes} disabled={closesAt === ""} onChange={(event) => { onChange({ closes: event.target.checked }); }} />
            it finishes by then
          </label>
        </span>
      </FieldBox>
    </div>
  );
}
