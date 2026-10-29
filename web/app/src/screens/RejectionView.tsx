// A refused write in words (A15), the one place every screen says it: what intervened, each
// rule broken, and what can be done about it. Also the words for a record a
// stale refusal names.
import type { Schema } from "@cairn/client";
import type { ReactNode } from "react";

import { violationNode, type Rejection, type Violation } from "../data/writes.ts";
import { useSession } from "../data/react.ts";
import { fieldWords } from "../status/words.ts";
import { Button } from "../ui/kit.tsx";

type RecordKey = Schema<"RecordKey">;


/** A field of a node an edit names, when the edit names one. */
function fieldOf(edit: unknown): string | undefined {
  return typeof edit === "object" && edit !== null && "field" in edit && typeof edit.field === "string" ? edit.field : undefined;
}

/** A record a touched set names, in words when it is a node of a journey or one of its fields; none for a record a person has no word for. */
export function describeRecord(key: RecordKey, titleOf: (journey: string, node: string) => string): string | undefined {
  if (!("in_graph" in key) || typeof key.in_graph.key !== "object") {
    return undefined;
  }
  const { graph } = key.in_graph;
  // A route draft's titles are not held by the tab, so a record of one has no words here.
  if (!("journey" in graph)) {
    return undefined;
  }
  const node = (key: string) => titleOf(graph.journey, key);
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
  const [name, value] = Object.entries(inner as Record<string, unknown>)[0] ?? ["", undefined];
  const about = typeof value === "string" ? value : typeof value === "object" && value !== null ? Object.values(value).find((each): each is string => typeof each === "string") : undefined;
  return about?.startsWith("n_") === true ? `${name.replaceAll("_", " ")} of "${node(about)}"` : undefined;
}

/** The words for the records a touched set names, by the titles of the journeys this tab holds; the ones with no words left out. */
export function useRecordWords(): (keys: readonly RecordKey[]) => string[] {
  const { titleOf } = useSession();
  return (keys) => [...new Set(keys.flatMap((key) => describeRecord(key, titleOf) ?? []))];
}

/** One rule an invalid change breaks (A15), in words: the engine's message, led by the node's title when it is about one. */
export function ViolationLine({ violation, titleOf, id }: { violation: Violation; titleOf?: ((node: string) => string) | undefined; id?: string | undefined }) {
  const node = violationNode(violation);
  return (
    <span id={id} className="author-problem" role="alert" data-testid="violation" data-code={violation.code} data-node={node}>
      {node === undefined || titleOf === undefined ? null : <strong>{titleOf(node)}: </strong>}
      {violation.message}
    </span>
  );
}

/** Every rule an invalid change breaks (A15), one line each. */
export function ViolationLines({ violations, titleOf, idPrefix }: { violations: readonly Violation[]; titleOf?: ((node: string) => string) | undefined; idPrefix?: string | undefined }) {
  return violations.map((violation, at) => <ViolationLine key={at} violation={violation} titleOf={titleOf} id={violationId(idPrefix, at)} />);
}

/** The id of the `at`th line of a list whose lines are pointed to from a control (`idPrefix`); none when nothing points to them. */
export function violationId(idPrefix: string | undefined, at: number): string | undefined {
  return idPrefix === undefined ? undefined : `${idPrefix}-v${String(at)}`;
}

/** A refusal's first sentence: the fact in bold, its reason in plain type. */
function NotSaved({ children }: { children: string }) {
  return <span><strong>Not saved:</strong> {children}</span>;
}

interface RejectionProps {
  rejection: Rejection;
  /** What a stale refusal says after "Not saved:", where the screen knows more than "changed first" (a create that met the id in use). */
  staleHeading?: string | undefined;
  /** What an invalid refusal says after "Not saved:" where one reason is the whole of it (checklist items not ticked), in place of "the change breaks a rule.". */
  invalidHeading?: string | undefined;
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
export function RejectionView({ rejection, onRebase, onDismiss, titleOf, omit, extras, staleHeading, invalidHeading, children }: RejectionProps) {
  const words = useRecordWords();
  let body: ReactNode;
  if (rejection.rejection === "stale") {
    const records = words(rejection.intervening);
    body = (
      <>
        <NotSaved>{staleHeading ?? "someone else changed this first."}</NotSaved>
        {records.length === 0 ? null : <span>What they changed: {records.join(", ")}</span>}
      </>
    );
  } else if (rejection.rejection === "invalid") {
    body = (
      <>
        <NotSaved>{invalidHeading ?? "the change breaks a rule."}</NotSaved>
        <ViolationLines violations={omit === undefined ? rejection.violations : rejection.violations.filter((violation) => !omit(violation))} titleOf={titleOf} />
      </>
    );
  } else {
    body = <NotSaved>that change was already sent once, so it was not sent again.</NotSaved>;
  }
  return (
    <div className="panel panel-bad stack" role="alert" data-testid="refused" data-rejection={rejection.rejection}>
      <span className="label">Error</span>
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
