// Drawing an edge on the canvas (A3): an author starts from a node ("draw a requirement"),
// then picks on the canvas the node it requires. The drawing is a machine the page owns
// (edge-drawing.ts); this is its React side: the hook the page calls, the key it listens
// to while drawing, and the banner that says what is being drawn and why a pick was refused or
// did not land. The write a pick makes is the edit manager's; the banner reads it through
// `useWriteById`, and the sync chip shows it too.
import { useActorRef, useSelector } from "@xstate/react";
import { createContext, useContext, useEffect, useMemo } from "react";

import { useSession, useCaller, useWriteById } from "../data/react.ts";
import { currentAddress } from "../data/session.ts";
import { reasonOf } from "../data/writes.ts";
import { ViolationLines } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Button } from "../ui/kit.tsx";
import { overlayOpen, typing } from "../ui/typing.ts";
import { drawingView, edgeDrawing, type DrawingView, type KeySource, type Refusal } from "./edge-drawing.ts";
import { titleIn, type Tree } from "./graph.ts";
import { domainOf, type Authored } from "./target.ts";
import { LOCAL_USER, previewOf } from "./write.ts";

/** An edge being drawn: what it is, and how to start or stop it. */
export interface Connecting {
  view: DrawingView;
  start: (from: string) => void;
  stop: () => void;
}

export const ConnectContext = createContext<Connecting | undefined>(undefined);

/** The page's edge drawing, if the page draws edges. */
export function useConnecting(): Connecting | undefined {
  return useContext(ConnectContext);
}

/**
 * Escape, heard on the window before any of the page's own Escape handlers (they listen on the
 * document, which comes after), unless a field has focus or a popover or dialog is open: those
 * take it first. The drawing claims the key it uses, so the node stays open and the mode stays on.
 */
export const windowKeys: KeySource = {
  listen: (cancel) => {
    const heard = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented && !typing(event.target) && !overlayOpen()) {
        event.preventDefault();
        cancel();
      }
    };
    globalThis.addEventListener("keydown", heard, { capture: true });
    return () => {
      globalThis.removeEventListener("keydown", heard, { capture: true });
    };
  },
};

/** The page's side: the drawing's machine, and the edge made when a card is picked. */
export interface EdgeDrawing extends Connecting {
  /** A card picked while drawing: true when the pick was taken (even a refused one), so the click opens nothing. */
  pick: (to: string) => boolean;
  tree: Tree | undefined;
}

/** The page's edge drawing for the graph `authored`; undefined `authored` is a page with nothing to author. */
export function useEdgeDrawing(authored: Authored | undefined): EdgeDrawing {
  const session = useSession();
  const { caller } = useCaller();
  const actor = useActorRef(edgeDrawing, {
    input: {
      edits: session.edits,
      preview: (graph, mutations, user) => previewOf({ deriver: session.deriver, user, authored: graph, mutations }),
      compatible: () => !session.skewed,
      keys: windowKeys,
    },
  });
  const snapshot = useSelector(actor, (each) => each);
  const view = useMemo(() => drawingView(snapshot), [snapshot]);
  // Leaving edit mode, or opening another graph, stops the drawing; the write it sent goes on.
  const domain = authored === undefined ? undefined : domainOf(authored);
  useEffect(() => {
    actor.send({ type: "cancel" });
  }, [actor, domain]);
  return {
    view,
    tree: authored?.tree,
    start: (from) => {
      actor.send({ type: "start", from });
    },
    stop: () => {
      actor.send({ type: "cancel" });
    },
    pick: (to) => {
      // The actor, not the rendered view: two clicks may come before a render, and the second is swallowed too.
      if (authored === undefined || !actor.getSnapshot().matches("active")) {
        return false;
      }
      actor.send({ type: "pick", to, authored, user: caller?.user ?? LOCAL_USER, address: currentAddress() });
      return true;
    },
  };
}

/** Why the last pick sent nothing, in words. */
function RefusalLines({ refused }: { refused: Refusal }) {
  if (refused.by === "preview" && refused.rejection.rejection === "invalid") {
    return <ViolationLines violations={refused.rejection.violations} />;
  }
  const words =
    refused.by === "containment"
      ? refused.message
      : refused.by === "preview"
        ? reasonOf(refused.rejection)
        : "This requirement was already sent and is not confirmed yet; see the sync chip.";
  return (
    <span className="author-problem" role="alert" data-testid="edge-refused">
      {words}
    </span>
  );
}

/** The banner while an edge is drawn: what to pick, what is being sent, and why a pick was refused or did not land. */
export function DrawingBanner({ drawing }: { drawing: EdgeDrawing }) {
  const { view, tree, stop } = drawing;
  const write = useWriteById(view.state === "choosing" || view.state === "saving" ? view.sent?.id : undefined);
  if (view.state === "inactive" || tree === undefined) {
    return null;
  }
  const stopButton = (
    <Button
      onClick={() => {
        stop();
        // The button that started the drawing, if it is on the page, takes the focus back once it can.
        requestAnimationFrame(() => {
          document.querySelector<HTMLElement>('[data-testid="draw-edge"]')?.focus();
        });
      }}
    >
      Stop drawing
    </Button>
  );
  const from = <strong>{titleIn(tree, view.from)}</strong>;
  return (
    <div className="banner stack drawing-banner" role="status" data-testid="drawing-edge" data-from={view.from} data-state={view.state}>
      <span className="row">
        {view.state === "choosing" && view.sent !== undefined && write !== undefined ? (
          <span className="drawing-words">
            Not added yet: {from} requires <strong>{titleIn(tree, view.sent.to)}</strong>. You can pick another node.
          </span>
        ) : null}
        {view.state === "choosing" && (view.sent === undefined || write === undefined) ? <span className="drawing-words">Pick the node that {from} requires.</span> : null}
        {view.state === "checking" ? (
          <span className="drawing-words">
            Checking: {from} requires <strong>{titleIn(tree, view.to)}</strong>...
          </span>
        ) : null}
        {view.state === "saving" ? (
          <span className="drawing-words">
            Saving: {from} requires <strong>{titleIn(tree, view.to)}</strong>. It saves even if you stop.
          </span>
        ) : null}
        {stopButton}
      </span>
      {view.state === "choosing" && view.refused !== undefined ? <RefusalLines refused={view.refused} /> : null}
      {write === undefined ? null : <WriteStatus view={write} layout="line" />}
    </div>
  );
}
