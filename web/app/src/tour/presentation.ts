// PRD: C20.
// What the overlay asks of the canvas: keep these cards in view, in this room. The overlay measures
// where its callout will sit and what is left of the canvas around it; the canvas, which owns its
// camera, frames the cards there. One request per state of a step (its id changes with the state),
// and none outside a graph step. Scrollers need no request: the overlay scrolls them itself.
import { createContext, useContext } from "react";

import type { Rect } from "../ui/place.ts";

export interface Framing {
  /** A new id is a new request; the canvas frames once per id. */
  id: number;
  /** The cards to keep in view, the first always, the rest while they stay readable together. */
  cards: readonly string[];
  /** The part of the window the cards may take, in the window's coordinates: the canvas less the room the callout keeps. */
  room: Rect;
  /** The canvas has framed the cards (once it is drawn and has them): what the overlay waits for before it judges where its callout fits. */
  applied: () => void;
}

export const FramingContext = createContext<Framing | undefined>(undefined);

/** The overlay's current request of the canvas, if there is one. */
export function useFraming(): Framing | undefined {
  return useContext(FramingContext);
}
