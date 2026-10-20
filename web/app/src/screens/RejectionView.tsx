// A refused write in words (A15), the one place every screen says it: what intervened, each
// rule broken without its code, and what can be done about it. Also the words for a record a
// stale refusal names.
import type { Schema } from "@cairn/client";
import type { ReactNode } from "react";

import { violationNode, violationWords, type Rejection, type Violation } from "../data/writes.ts";
import { useSession } from "../data/react.ts";
import { fieldWords } from "../status/words.ts";
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
    return `${fieldWords(inner.node_field.field)} of "${node(inner.node_field.node)}"`;
  }
  if ("local_edit" in inner) {
    const field = fieldOf(inner.local_edit.edit);
    return field === undefined ? `"${node(inner.local_edit.node)}"` : `${fieldWords(field)} of "${node(inner.local_edit.node)}"`;
  }
  const [name, value] = Object.entries(inner as Record<string, unknown>)[0] ?? ["record", undefined];
  return typeof value === "string" && value.startsWith("n_") ? `${name.replaceAll("_", " ")} of "${node(value)}"` : name.replaceAll("_", " ");
}

/** The words for a record a touched set names, by the titles of the journeys this tab holds. */
export function useRecordWords(): (key: RecordKey) => string {
  const { titleOf } = useSession();
  return (key) => describeRecord(key, titleOf);
}

/** One rule an invalid change breaks (A15), in words: the engine's message without its rule code, led by the node's title when it is about one. */
export function ViolationLine({ violation, titleOf }: { violation: Violation; titleOf?: ((node: string) => string) | undefined }) {
  const node = violationNode(violation);
  return (
    <span className="author-problem" role="alert" data-testid="violation" data-code={violation.code} data-node={node}>
      {node === undefined || titleOf === undefined ? null : <strong>{titleOf(node)}: </strong>}
      {violationWords(violation)}
    </span>
  );
}

/** Every rule an invalid change breaks (A15), one line each. */
export function ViolationLines({ violations, titleOf }: { violations: readonly Violation[]; titleOf?: ((node: string) => string) | undefined }) {
  return violations.map((violation, at) => <ViolationLine key={at} violation={violation} titleOf={titleOf} />);
}

interface RejectionProps {
  rejection: Rejection;
  /** Offers to keep the edit on the current version, after a stale refusal. */
  onRebase?: (() => void) | undefined;
  /** Offers to drop the refused change. */
  onDismiss?: (() => void) | undefined;
  /** Names the node a violation is about, in front of it. */
  titleOf?: ((node: string) => string) | undefined;
  /** Violations another part of the screen shows (a contradictory chain's moves), left out of the list. */
  omit?: ((violation: Violation) => boolean) | undefined;
  /** What the screen offers against the refusal beneath its words (a resolution, a bypass). */
  extras?: ReactNode;
  /** What the screen offers beside Dismiss (A11: discard the existing draft). */
  children?: ReactNode;
}

/**
 * A refused write, shown whole (A15) wherever it is refused: a stale one with what intervened
 * (H5: "otherwise the intervening changes are shown and the user retries"), an invalid one with
 * each rule it breaks, and a change sent twice.
 */
export function RejectionView({ rejection, onRebase, onDismiss, titleOf, omit, extras, children }: RejectionProps) {
  const words = useRecordWords();
  let body: ReactNode;
  if (rejection.rejection === "stale") {
    const records = [...new Set(rejection.intervening.map(words))];
    body = (
      <>
        <strong>Not saved: someone changed this since you started editing.</strong>
        {records.length === 0 ? null : <span>What they changed: {records.join(", ")}</span>}
      </>
    );
  } else if (rejection.rejection === "invalid") {
    body = (
      <>
        <strong>Not saved: the change breaks a rule.</strong>
        <ViolationLines violations={omit === undefined ? rejection.violations : rejection.violations.filter((violation) => !omit(violation))} titleOf={titleOf} />
      </>
    );
  } else {
    body = <strong>Not saved: that change was already sent once, so it was not sent again.</strong>;
  }
  return (
    <div className="callout callout-bad stack" role="alert" data-testid="refused" data-rejection={rejection.rejection}>
      {body}
      {extras}
      <span className="row">
        {onRebase === undefined || rejection.rejection !== "stale" ? null : <Button onClick={onRebase}>Keep my edit on the current version</Button>}
        {children}
        {onDismiss === undefined ? null : <Button onClick={onDismiss}>Dismiss</Button>}
      </span>
    </div>
  );
}
