// What the viewer is pointing at beside the canvas, by pointer or keyboard (hover or focus): a node
// (a row of the Stages list) or a linked date (a row of the inspector's dates). The lists and the
// canvas are separate trees under the shell, so the shell holds it and the canvas reads it: the
// pointed node is outlined as the ends of a line under the pointer are, and the date lines standing
// for the pointed relationship stay in ink while the others step back. Hover and focus are held
// apart, so leaving one keeps the other's row pointed at. Outside the shell nothing is pointed at.
import { createContext, type ReactNode, use, useCallback, useEffect, useMemo, useState } from "react";

import type { DateLine, DateLink } from "./model.ts";

/** One relationship, as the inspector's row and the canvas's line entries name it. */
export function dateLinkId(link: DateLink): string {
  return `${link.node}|${link.source}|${link.kind}|${"rule" in link ? link.rule : ""}`;
}

/** What a row points at: a node by its key, or a date relationship by its id. */
export interface Target {
  kind: "node" | "date";
  id: string;
}

type How = "hover" | "focus";

const same = (one: Target | undefined, other: Target | undefined) => one?.kind === other?.kind && one?.id === other?.id;

interface Pointing {
  pointed: Target | undefined;
  /** Hover or focus now rests on `target`; or, with none, leaves it if it was the one resting there. */
  point: (how: How, target: Target | undefined, from?: Target) => void;
}

const NOTHING: Pointing = { pointed: undefined, point: () => undefined };
const PointingContext = createContext<Pointing>(NOTHING);

export function PointedProvider({ children }: { children: ReactNode }) {
  const [held, hold] = useState<{ hover: Target | undefined; focus: Target | undefined }>({ hover: undefined, focus: undefined });
  const point = useCallback((how: How, target: Target | undefined, from?: Target) => {
    hold((now) => (target === undefined && from !== undefined && !same(now[how], from) ? now : same(now[how], target) ? now : { ...now, [how]: target }));
  }, []);
  const pointed = held.hover ?? held.focus;
  const value = useMemo(() => ({ pointed, point }), [pointed, point]);
  return <PointingContext value={value}>{children}</PointingContext>;
}

/** The node being pointed at, if any. */
export function usePointedNode(): string | undefined {
  const { pointed } = use(PointingContext);
  return pointed?.kind === "node" ? pointed.id : undefined;
}

/** The date relationship being pointed at, if any. */
export function usePointedDate(): string | undefined {
  const { pointed } = use(PointingContext);
  return pointed?.kind === "date" ? pointed.id : undefined;
}

/**
 * The events of a row that points at `target`: its hover and its keyboard focus. A row that goes
 * away while pointed at lets go, and leaving one never clears what another row points at.
 */
export function usePointRow(target: Target): { onPointerEnter: () => void; onPointerLeave: () => void; onFocus: () => void; onBlur: () => void } {
  const { point } = use(PointingContext);
  const { kind, id } = target;
  const [enter, leave, focus, blur] = useMemo(() => {
    const mine = { kind, id };
    return [() => { point("hover", mine); }, () => { point("hover", undefined, mine); }, () => { point("focus", mine); }, () => { point("focus", undefined, mine); }] as const;
  }, [point, kind, id]);
  useEffect(
    () => () => {
      leave();
      blur();
    },
    [leave, blur],
  );
  return { onPointerEnter: enter, onPointerLeave: leave, onFocus: focus, onBlur: blur };
}

/** Whether `line` steps back while relationship `pointed` is pointed at: it stands for no such relationship. */
export function stepsBack(line: DateLine, pointed: string | undefined): boolean {
  return pointed !== undefined && !line.entries.some(({ link }) => dateLinkId(link) === pointed);
}
