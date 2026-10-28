// C1, C8: whether the selected node's date lines are drawn. They are drawn only while its Dates
// section is open and the inspector is showing it, so the section is the one switch for them: the viewer's own fold
// (remembered per kind of node like every section), else open on a shortfall or when overdue.
import { useFrameState } from "../shell/frame.tsx";
import { foldKey, useFold } from "./folds.ts";
import { nodeDetail, type NodeDetail, type Ready } from "./model.ts";

/** Whether a node's Dates section starts open, when the viewer has not chosen: on a shortfall or when overdue. */
export function datesStartOpen(detail: NodeDetail): boolean {
  return detail.derived.dates.shortfall != null || detail.derived.overdue === true;
}

/** The key a Dates section of a node of `kind` is remembered under. */
export function datesFold(kind: string): string {
  return foldKey(kind, "dates");
}

/**
 * Whether the Dates section of a node of `kind` is open and the inspector showing it: not folded, and the inspector
 * showing node detail (not hidden, on another tab, or a sheet at its peek). Outside the shell there is no
 * frame to hide it.
 */
export function useDatesShown(kind: string | undefined, startOpen: boolean): boolean {
  const open = useFold(kind === undefined ? undefined : datesFold(kind), startOpen);
  const showing = useFrameState()?.inspectorShowing ?? true;
  return kind !== undefined && open && showing;
}

/** `useDatesShown` for a journey's node: none when no single node is selected. */
export function useJourneyDatesShown(view: Ready, key: string | undefined): boolean {
  const detail = key === undefined ? undefined : nodeDetail(view, key);
  return useDatesShown(detail?.node.kind, detail === undefined ? false : datesStartOpen(detail));
}
