// Drawing an edge on the canvas (A3): an author starts from a node ("draw a requirement"),
// then picks on the canvas the node it requires. The page holds which node an edge is being
// drawn from and turns the next card picked into the edge, refused with its reason when the
// two are already related by containment. Escape or Cancel stops drawing.
import { createContext, useContext, useEffect, useRef, useState } from "react";

import { reasonOf } from "../data/writes.ts";
import { Button } from "../ui/kit.tsx";
import { edgeRefusal, titleIn } from "./graph.ts";
import type { Authored } from "./target.ts";
import { useAuthorWrite, type AuthorWrite } from "./write.ts";

/** An edge being drawn: the dependent it starts from, and how to start or stop. */
export interface Connecting {
  from: string | undefined;
  start: (from: string) => void;
  cancel: () => void;
}

export const ConnectContext = createContext<Connecting | undefined>(undefined);

/** The page's edge drawing, if the page draws edges. */
export function useConnecting(): Connecting | undefined {
  return useContext(ConnectContext);
}

/**
 * What to tell the author when a picked card's write did not land and nothing else says why: a
 * rule violation is listed by the banner itself, so it is not repeated. An unanswered write is
 * unconfirmed, not unsent (it may have committed). Drawing goes on, so the pick can be retried.
 */
export function pickProblem(dropped: boolean, write: Pick<AuthorWrite, "failed" | "unanswered">): string | undefined {
  const { failed, unanswered } = write;
  if (!dropped || failed?.rejection.rejection === "invalid") {
    return undefined;
  }
  if (unanswered === "stopped") {
    return "Couldn't add the requirement: Cairn was updated, so the change was not sent. Reload; then try again.";
  }
  if (unanswered === "failed") {
    return "Couldn't confirm the requirement was added: no answer came back, so it may have been saved. Check the list before trying again.";
  }
  return `Couldn't add the requirement: ${failed === undefined ? "the change did not go through" : reasonOf(failed.rejection).replace(/\.$/, "")}. Try again.`;
}

/** The page's side: which node an edge is drawn from, and the edge made when a card is picked. */
export function useEdgeDrawing(authored: Authored | undefined) {
  const [from, setFrom] = useState<string | undefined>(undefined);
  const [refusal, setRefusal] = useState<string | undefined>(undefined);
  const [dropped, setDropped] = useState(false);
  // A write's answer belongs to the drawing attempt that sent it: starting, cancelling or picking again supersedes it.
  const attempt = useRef(0);
  const write = useAuthorWrite(authored ?? FALLBACK);
  useEffect(() => {
    const stop = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setFrom(undefined);
      }
    };
    globalThis.addEventListener("keydown", stop);
    return () => {
      globalThis.removeEventListener("keydown", stop);
    };
  }, []);
  const connecting: Connecting = {
    from,
    start: (next) => {
      attempt.current += 1;
      setFrom(next);
      setRefusal(undefined);
      setDropped(false);
    },
    cancel: () => {
      attempt.current += 1;
      setFrom(undefined);
      setRefusal(undefined);
      setDropped(false);
    },
  };
  /** A card picked while drawing: the edge from the drawn node to it. True when the pick was taken. */
  const pick = (to: string): boolean => {
    if (from === undefined || authored === undefined) {
      return false;
    }
    const refused = edgeRefusal(authored.tree, from, to);
    if (refused !== undefined) {
      setRefusal(refused);
      return true;
    }
    setRefusal(undefined);
    setDropped(false);
    attempt.current += 1;
    const mine = attempt.current;
    void write.run([{ op: "add_edge", edge: { node: from, requires: to } }]).then((landed) => {
      if (mine !== attempt.current) {
        return;
      }
      if (landed) {
        setFrom(undefined);
      } else {
        setDropped(true);
      }
    });
    return true;
  };
  return { connecting, pick, refusal, problem: pickProblem(dropped, write), failed: write.failed, tree: authored?.tree };
}

/** The banner while an edge is drawn: what to pick, and why the last pick was refused. */
export function DrawingBanner({ drawing }: { drawing: ReturnType<typeof useEdgeDrawing> }) {
  const { connecting, refusal, problem, failed, tree } = drawing;
  if (connecting.from === undefined || tree === undefined) {
    return null;
  }
  const violations = failed?.rejection.rejection === "invalid" ? failed.rejection.violations : [];
  return (
    <div className="banner banner-warn stack" role="status" data-testid="drawing-edge" data-from={connecting.from}>
      <span className="row">
        <span>
          Pick the node that <strong>{titleIn(tree, connecting.from)}</strong> requires.
        </span>
        <Button onClick={connecting.cancel}>Cancel</Button>
      </span>
      {refusal === undefined ? null : (
        <span className="author-problem" role="alert" data-testid="edge-refused">
          {refusal}
        </span>
      )}
      {problem === undefined ? null : (
        <span className="author-problem" role="alert" data-testid="edge-unlanded">
          {problem}
        </span>
      )}
      {violations.map((violation, at) => (
        <span key={at} className="author-problem" role="alert" data-testid="violation" data-code={violation.code}>
          {violation.message}
        </span>
      ))}
    </div>
  );
}

/** A graph with nothing in it, for the hook's write path while the page has none to author. */
const FALLBACK: Authored = {
  target: { route: "none" },
  graph: {},
  tree: { graph: {}, byKey: new Map(), children: new Map() },
  revision: 0,
  deploymentRevision: 0,
  deployment: { revision: 0 },
  today: "",
  lineage: undefined,
  route: undefined,
};
