// The graph's selection: one set, no mode. One member is the open node (the address), inspected
// and traced; several are held here, listed and acted on in the inspector, with no trace.
// Everything that adds to or removes from the set ends the same way: a set of one is the
// address's node again, an empty one is no node, and two or more are kept for as long as the
// address stays on the node they were made at (any other way of opening a node replaces them).
import { useState } from "react";

import { useFrameActions } from "../shell/frame.tsx";

export interface Selection {
  /** Who is selected, in the order selected. */
  members: readonly string[];
  /** A card plain-clicked: it alone. */
  pick: (key: string) => void;
  /** A card added to the set, or removed from it. */
  toggle: (key: string) => void;
  /** The set made `keys` (a box drawn over cards). */
  set: (keys: readonly string[]) => void;
  /** Nobody. */
  clear: () => void;
  /** The several held are let go, because the address is about to open a node. */
  release: () => void;
}

interface Held {
  /** The node the address is on while they are held. */
  at: string | undefined;
  keys: readonly string[];
  /** Whether the address has reached `at` yet: the set is let go once it leaves it, not before it arrives. */
  landed: boolean;
}

const NONE: Held = { at: undefined, keys: [], landed: false };

/** The selection of a canvas whose address is on `selected`; `go` opens a node there, or none. */
export function useSelection(selected: string | undefined, go: (node: string | undefined) => void): Selection {
  const frame = useFrameActions();
  const [held, setHeld] = useState<Held>(NONE);
  if (held.keys.length > 0) {
    if (held.at === selected && !held.landed) {
      setHeld({ ...held, landed: true });
    } else if (held.landed && held.at !== selected) {
      // Esc, Back, a list row: any other way of opening a node replaces the set, now and for good.
      setHeld(NONE);
    }
  }
  const members = held.keys.length >= 2 && held.at === selected ? held.keys : selected === undefined ? [] : [selected];
  const set = (next: readonly string[]) => {
    if (next.length < 2) {
      setHeld(NONE);
      if (next[0] !== selected) {
        go(next[0]);
      }
      return;
    }
    // The address stays on a member, so leaving it (Esc, another node) lets the set go.
    const at = selected !== undefined && next.includes(selected) ? selected : next[0];
    setHeld({ at, keys: next, landed: at === selected });
    frame?.show("inspector");
    if (at !== selected) {
      go(at);
    }
  };
  return {
    members,
    pick: (key) => {
      setHeld(NONE);
      go(key);
    },
    toggle: (key) => { set(members.includes(key) ? members.filter((each) => each !== key) : [...members, key]); },
    set,
    clear: () => {
      setHeld(NONE);
      go(undefined);
    },
    release: () => { setHeld(NONE); },
  };
}
