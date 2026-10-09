// C12, C8 (design 6.7): what each answer of a decision would do, node by node: for each choice,
// the nodes it brings in, drops, and leaves to be decided later, each a link with its state, and
// the role it fills and the milestone it pins. It replaces the decision view's table, one click
// from the form that shows the same effects as counts.
import type { Schema } from "@cairn/client";

import { useProjected } from "../canvas/hooks.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge } from "../ui/kit.tsx";
import { foldKey } from "./folds.ts";
import { nodeOf, type NodeDetail, type Ready } from "./model.ts";
import { NodeLink, Section } from "./parts.tsx";
import { roleTitle } from "./sections.tsx";

type ChoiceEffect = Schema<"ChoiceEffect">;

/** The title of the answer a choice is, for "If Partner". */
function choiceName(effect: ChoiceEffect, detail: NodeDetail): string {
  if ("boolean" in effect.answer) {
    return effect.answer.boolean ? "yes" : "no";
  }
  const found = (detail.node.choices ?? []).find((choice) => (typeof choice === "string" ? choice : choice.id) === effect.choice);
  return found === undefined ? (effect.choice ?? "this answer") : typeof found === "string" ? found : found.title;
}

function Group({ view, label, keys, total }: { view: Ready; label: string; keys: string[]; total: number }) {
  if (total === 0) {
    return null;
  }
  return (
    <div className="stack">
      <span className="muted small">
        {label} ({total})
      </span>
      <ul className="detail-list">
        {keys.map((key) => {
          const derived = view.derived.nodes[key];
          return (
            <li key={key} data-testid="affected" data-node={key} data-relevance={derived?.relevance.value}>
              <NodeLink view={view} node={key} />{" "}
              {derived === undefined ? null : (
                <Badge tone={statusTone(derived.display_state)}>{statusWord(derived.display_state, nodeOf(view, key)?.kind ?? "action")}</Badge>
              )}
            </li>
          );
        })}
      </ul>
      {total > keys.length ? <span className="muted small">and {total - keys.length} more</span> : null}
    </div>
  );
}

export function Affects({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const { node } = detail;
  const projected = useProjected(view, { projection: "answer_effects", key: node.key });
  const effects = projected.value;
  if (effects == null) {
    return null;
  }
  const choices = (effects.choices ?? []).filter((effect) => effect.current !== true);
  const touched = new Set(choices.flatMap((effect) => [...(effect.brings_in.nodes ?? []), ...(effect.drops.nodes ?? []), ...(effect.decided_later.nodes ?? [])]));
  const fills = effects.fills_role ?? undefined;
  const pins = effects.pins ?? undefined;
  if (touched.size === 0 && fills === undefined && pins === undefined) {
    return null;
  }
  return (
    <Section title="Affects" summary={String(touched.size)} fold={foldKey(node.kind, "affects")} testId="decision-affects">
      {choices.map((effect) => (
        <div key={JSON.stringify(effect.answer)} className="stack" data-testid="effect-group">
          <strong>If {choiceName(effect, detail)}</strong>
          <Group view={view} label="Brings in" keys={effect.brings_in.nodes ?? []} total={effect.brings_in.total} />
          <Group view={view} label="Drops" keys={effect.drops.nodes ?? []} total={effect.drops.total} />
          <Group view={view} label="Decided later" keys={effect.decided_later.nodes ?? []} total={effect.decided_later.total} />
        </div>
      ))}
      {pins === undefined ? null : (
        <span data-testid="pins" data-node={pins}>
          Pins <NodeLink view={view} node={pins} />.
        </span>
      )}
      {fills === undefined ? null : <span data-testid="fills">Fills the role {roleTitle(view, fills)}.</span>}
    </Section>
  );
}
