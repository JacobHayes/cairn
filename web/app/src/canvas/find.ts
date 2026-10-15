// The graph's find (2.3): a search names a node, which opens on the canvas with its containers
// expanded, and the canvas centres it once it is drawn.
import { useState } from "react";
import { useNavigate } from "react-router";

import { nodeOf, type Ready } from "../detail/model.ts";
import { ancestorsOf } from "./ladder.ts";
import { canvasPath, revealing, type CanvasView } from "./settings.ts";

/** The graph's find (2.3): the first node whose title holds `text`. */
function findNode(ready: Ready, text: string): string | undefined {
  const wanted = text.trim().toLowerCase();
  const nodes = ready.journey.graph.nodes ?? [];
  return (nodes.find((node) => node.title.toLowerCase() === wanted) ?? nodes.find((node) => node.title.toLowerCase().includes(wanted)))?.key;
}

/** The find: it reads the centring request from the navigation. `missing` is the search that named no node. */
export function useFind(ready: Ready, view: CanvasView, release: () => void) {
  const navigate = useNavigate();
  const [missing, setMissing] = useState<string | undefined>(undefined);
  const journey = ready.journey.header.id;
  const centre = (key: string) => {
    const found = nodeOf(ready, key);
    if (found !== undefined) {
      // Opening a node replaces a selection of several, even when it is one of them.
      release();
      void navigate(canvasPath(journey, revealing(view, { kind: found.kind, ancestors: ancestorsOf(ready.journey.graph.nodes ?? [], key) }, ready.derived.nodes[key]?.display_state), key), { state: { reveal: { key, how: "centre" } } });
    }
  };
  const find = (text: string) => {
    const key = text === "" ? undefined : findNode(ready, text);
    setMissing(text !== "" && key === undefined ? text : undefined);
    if (key !== undefined) {
      centre(key);
    }
  };
  return { find, missing };
}

