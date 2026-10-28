// C1: the linked date the viewer is pointing at in the inspector (hover or keyboard focus on its
// row). The inspector and the canvas are separate trees, so it is a small store the canvas's date
// lines subscribe to: the lines standing for the pointed relationship stay in ink and the others
// step back. Hover and focus are held apart, so leaving one keeps the other's row pointed at.
import { useSyncExternalStore } from "react";

import type { DateLine, DateLink } from "./model.ts";

/** One relationship, as the inspector's row and the canvas's line entries name it. */
export function dateLinkId(link: DateLink): string {
  return `${link.node}|${link.source}|${link.kind}|${"rule" in link ? link.rule : ""}`;
}

const held: { hover: string | undefined; focus: string | undefined } = { hover: undefined, focus: undefined };
let pointed: string | undefined;
const listeners = new Set<() => void>();

/** Hover or focus now rests on the row of relationship `id`, or on none. */
export function pointAtDate(how: "hover" | "focus", id: string | undefined): void {
  held[how] = id;
  const next = held.hover ?? held.focus;
  if (next === pointed) {
    return;
  }
  pointed = next;
  listeners.forEach((listen) => {
    listen();
  });
}

/** The relationship being pointed at, as it changes. */
export function usePointedDate(): string | undefined {
  return useSyncExternalStore(
    (listen) => {
      listeners.add(listen);
      return () => {
        listeners.delete(listen);
      };
    },
    () => pointed,
    () => undefined,
  );
}

/** Whether `line` steps back while relationship `pointed` is pointed at: it stands for no such relationship. */
export function stepsBack(line: DateLine, pointed: string | undefined): boolean {
  return pointed !== undefined && !line.entries.some(({ link }) => dateLinkId(link) === pointed);
}
