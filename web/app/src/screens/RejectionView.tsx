// A rejected write, shown whole (A15): a stale one with what intervened (H5: "otherwise the
// intervening changes are shown and the user retries"), an invalid one with its violations.
import type { Schema } from "@cairn/client";

import type { Rejection } from "../data/writes.ts";
import { Button } from "../ui/kit.tsx";

type RecordKey = Schema<"RecordKey">;

/** A field of a node an edit names, when the edit names one. */
function fieldOf(edit: unknown): string | undefined {
  return typeof edit === "object" && edit !== null && "field" in edit && typeof edit.field === "string" ? edit.field : undefined;
}

/** A record a touched set names, in words: a node's field, a node's own record, or the key itself. */
export function describeRecord(key: RecordKey): string {
  if (!("in_graph" in key) || typeof key.in_graph.key !== "object") {
    return JSON.stringify(key);
  }
  const inner = key.in_graph.key;
  if ("node" in inner) {
    return `node ${inner.node}`;
  }
  if ("node_field" in inner) {
    return `${inner.node_field.field} of node ${inner.node_field.node}`;
  }
  if ("local_edit" in inner) {
    const field = fieldOf(inner.local_edit.edit);
    return field === undefined ? `node ${inner.local_edit.node}` : `${field} of node ${inner.local_edit.node}`;
  }
  const [name, value] = Object.entries(inner as Record<string, unknown>)[0] ?? ["record", undefined];
  return typeof value === "string" ? `${name.replaceAll("_", " ")} of node ${value}` : JSON.stringify(key);
}

export function RejectionView({ rejection, onRebase }: { rejection: Rejection; onRebase: () => void }) {
  if (rejection.rejection === "stale") {
    const records = [...new Set(rejection.intervening.map(describeRecord))];
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
            <li key={at} className="mono">
              {violation.code}
            </li>
          ))}
        </ul>
      </div>
    );
  }
  return (
    <div className="callout callout-bad" role="alert">
      This patch id was already used for another change ({rejection.patch_id}).
    </div>
  );
}
