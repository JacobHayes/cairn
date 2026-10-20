// C10, C11, B6: one button for an action `offered()` names, drawn the same on a row, in the
// inspector and wherever else a node's actions appear: the same words (ACT_LABELS), the same
// one move. What needs more than one move opens a form that belongs to the surface, which the
// surface supplies as `open`.
import { useState } from "react";

import { useReviewing } from "../proposals/layer.ts";
import { Button, Field } from "../ui/kit.tsx";
import { transition, type Mutation, type Ready } from "./model.ts";
import { actLabel, unsnoozeOf, type Act } from "./offers.ts";
import type { NodeWrite } from "./write.ts";

/** What an action opens rather than sends: the done form, the break-down form, or the answer form. */
export type Opens = "evidence" | "break-down" | "decide";

/** What `act` sends for `key` as its one move, if it is one (the others open a form). */
export function actMutation(view: Ready, act: Act, key: string): Mutation | undefined {
  switch (act) {
    case "done":
      return transition(key, "complete");
    case "start":
      return transition(key, "start");
    case "reach":
    case "reach-now":
      return transition(key, "reach");
    case "mark-atomic":
      return { op: "set_atomic", node: key, atomic: true };
    case "unsnooze": {
      const holder = unsnoozeOf(view, key);
      return holder === undefined ? undefined : { op: "unsnooze", node: holder };
    }
    case "decide":
    case "done-with-evidence":
    case "break-down":
      return undefined;
  }
}

/** F1, D1: Mark reached with the date it was reached on, today unless changed. */
function Reach({ view, write, node, disabled }: { view: Ready; write: NodeWrite; node: string; disabled: boolean }) {
  const today = view.derived.today;
  const [date, setDate] = useState(today);
  return (
    <span className="row">
      <Button
        primary
        disabled={disabled || write.disabled || date === ""}
        onClick={() => void write.run([transition(node, "reach"), ...(date === today ? [] : [{ op: "set_recorded_date" as const, node, end: "finish" as const, date }])])}
      >
        {actLabel(view, node, "reach")}
      </Button>
      <Field type="date" aria-label="Reached on" title="Reached on" value={date} onChange={(event) => { setDate(event.target.value); }} />
    </span>
  );
}

/**
 * One of a node's action buttons. `waiting`: the action would be refused, so it is disabled.
 * `dated` lets a milestone be reached on another day than today, where there is room for the date.
 */
export function ActButton({ act, view, node, write, waiting, open, dated = false }: { act: Act; view: Ready; node: string; write: NodeWrite; waiting: boolean; open: (what: Opens) => void; dated?: boolean }) {
  // Under a proposal's layer the journey drawn is the one after it: acting on it would act on the present.
  const reviewing = useReviewing();
  const mutation = actMutation(view, act, node);
  if (reviewing !== undefined) {
    return null;
  }
  if (act === "reach" && dated) {
    return <Reach view={view} write={write} node={node} disabled={waiting} />;
  }
  const opens = act === "decide" || act === "break-down" || act === "done-with-evidence";
  const refused = waiting && (act === "done" || act === "reach");
  return (
    <Button
      primary={act === "done" || act === "done-with-evidence" || act === "reach" || act === "reach-now" || act === "decide" || act === "break-down"}
      disabled={refused || (act !== "decide" && act !== "break-down" && write.disabled)}
      data-testid={act === "break-down" ? "break-down" : undefined}
      onClick={() => {
        if (opens) {
          open(act === "done-with-evidence" ? "evidence" : act === "decide" ? "decide" : "break-down");
        } else if (mutation !== undefined) {
          void write.run([mutation]);
        }
      }}
    >
      {actLabel(view, node, act)}
    </Button>
  );
}
