// A rejected write, shown whole (A15): a stale one with what intervened (H5: "otherwise the
// intervening changes are shown and the user retries"), an invalid one with its violations.
import type { Schema } from "@cairn/client";

import type { Rejection } from "../data/writes.ts";
import { useSession } from "../data/react.ts";
import { Button } from "../ui/kit.tsx";

type RecordKey = Schema<"RecordKey">;

/** A route draft's titles are not held by the tab, so a record of one is named by where it is, never as removed. */
const IN_A_ROUTE = "an item of the route";

/** A field of a node an edit names, when the edit names one. */
function fieldOf(edit: unknown): string | undefined {
  return typeof edit === "object" && edit !== null && "field" in edit && typeof edit.field === "string" ? edit.field : undefined;
}

/** A record a touched set names, in words: a node's field or its own record by the node's title, or just what kind of record it is. */
export function describeRecord(key: RecordKey, titleOf: (journey: string, node: string) => string): string {
  if (!("in_graph" in key) || typeof key.in_graph.key !== "object") {
    return "other details";
  }
  const { graph } = key.in_graph;
  const node = (key: string) => ("journey" in graph ? titleOf(graph.journey, key) : IN_A_ROUTE);
  const inner = key.in_graph.key;
  if ("node" in inner) {
    return `"${node(inner.node)}"`;
  }
  if ("node_field" in inner) {
    return `${inner.node_field.field.replaceAll("_", " ")} of "${node(inner.node_field.node)}"`;
  }
  if ("local_edit" in inner) {
    const field = fieldOf(inner.local_edit.edit);
    return field === undefined ? `"${node(inner.local_edit.node)}"` : `${field.replaceAll("_", " ")} of "${node(inner.local_edit.node)}"`;
  }
  const [name, value] = Object.entries(inner as Record<string, unknown>)[0] ?? ["record", undefined];
  return typeof value === "string" && value.startsWith("n_") ? `${name.replaceAll("_", " ")} of "${node(value)}"` : name.replaceAll("_", " ");
}

/** The words for a record a touched set names, by the titles of the journeys this tab holds. */
export function useRecordWords(): (key: RecordKey) => string {
  const { titleOf } = useSession();
  return (key) => describeRecord(key, titleOf);
}

type Violations = Extract<Rejection, { rejection: "invalid" }>["violations"];

/** Every rule an invalid change breaks (A15), one line each, in the engine's words. */
export function ViolationLines({ violations }: { violations: Violations }) {
  return violations.map((violation, at) => (
    <span key={at} className="author-problem" role="alert" data-testid="violation" data-code={violation.code}>
      {violation.message}
    </span>
  ));
}

export function RejectionView({ rejection, onRebase }: { rejection: Rejection; onRebase: () => void }) {
  const words = useRecordWords();
  if (rejection.rejection === "stale") {
    const records = [...new Set(rejection.intervening.map(words))];
    return (
      <div className="callout callout-bad stack" role="alert" data-testid="conflict">
        <strong>Someone changed this since you started editing.</strong>
        <span>What they changed: {records.join(", ")}</span>
        <span className="row">
          <Button onClick={onRebase}>Keep my edit on the current version</Button>
        </span>
      </div>
    );
  }
  if (rejection.rejection === "invalid") {
    return (
      <div className="callout callout-bad stack" role="alert" data-testid="invalid">
        <strong>Not saved: the change breaks a rule.</strong>
        <ul>
          {rejection.violations.map((violation, at) => (
            <li key={at}>{violation.message}</li>
          ))}
        </ul>
      </div>
    );
  }
  return (
    <div className="callout callout-bad" role="alert">
      That change was already sent once, so it was not sent again.
    </div>
  );
}
