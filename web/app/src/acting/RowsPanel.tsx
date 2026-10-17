// The bounded list of rows a page shows (the Summary's sections, Home's journeys, the open
// proposals): a panel of rows (each an `li[data-row]` whose link carries `data-row-title`), the
// first five of them and "Show all" for the rest, optionally a fold of the same, and the keeping of the focus as rows move away (C18). The rows are rendered by the caller, so a
// panel can hold rows of several journeys; this owns only how many are drawn and where the focus
// goes. On paper every row is there and no control is.
import { Fragment, useEffect, useLayoutEffect, useRef, useState, type FocusEvent, type ReactNode } from "react";
import { flushSync } from "react-dom";

import { Button } from "../ui/kit.tsx";
import { NodePanel } from "./NodeItem.tsx";

/** How many rows a list shows before "Show all": the rest are on paper, and one click away. */
export const SHOWN_MAX = 5;

/**
 * One row of a panel. `key` names it across the page (a journey and a node, when a page lists
 * several journeys), and `render` draws it only when it is shown, so a long list reads five. It is
 * told whether the person asked to see the row: a row drawn only for paper offers no controls.
 */
export interface PanelRow {
  key: string;
  render: (shown: boolean) => ReactNode;
}

/**
 * Whether the page is being printed (or shown as print): the rows past the five shown are in
 * the page only then, and a fold is open, since it is the report (C18). `beforeprint` is
 * flushed at once so the paper has them; the media query also covers print emulation.
 */
function usePrinting(): boolean {
  const [printing, setPrinting] = useState(false);
  useEffect(() => {
    const media = globalThis.matchMedia("print");
    const on = () => { flushSync(() => { setPrinting(true); }); };
    const off = () => { setPrinting(false); };
    const changed = () => { setPrinting(media.matches); };
    globalThis.addEventListener("beforeprint", on);
    globalThis.addEventListener("afterprint", off);
    media.addEventListener("change", changed);
    changed();
    return () => {
      globalThis.removeEventListener("beforeprint", on);
      globalThis.removeEventListener("afterprint", off);
      media.removeEventListener("change", changed);
    };
  }, []);
  return printing;
}

/**
 * Keeps the focus in a list as its rows move: when the row that had it leaves (done, answered,
 * passed on, or put away by "Show fewer") the row now in its place takes it, or the panel when the
 * list empties. Nothing else moves the focus, so a person who went elsewhere is not pulled back.
 */
function useKeptFocus(rows: PanelRow[]) {
  const panel = useRef<HTMLDivElement>(null);
  const focused = useRef<{ key: string; at: number } | undefined>(undefined);
  const keys = rows.map((row) => row.key).join("\n");
  useEffect(() => {
    const held = focused.current;
    if (held === undefined || keys.split("\n").includes(held.key)) {
      return;
    }
    focused.current = undefined;
    const active = document.activeElement;
    if (active !== null && active !== document.body) {
      return;
    }
    const titles = panel.current?.querySelectorAll<HTMLElement>("[data-row-title]");
    const next = titles === undefined || titles.length === 0 ? undefined : titles[Math.min(held.at, titles.length - 1)];
    (next ?? panel.current)?.focus();
  }, [keys]);
  // A panel that leaves with the focus inside (its last row was done) hands it to another list's
  // first row, or the page, before it goes: the cleanup runs while its element is still attached.
  useLayoutEffect(() => {
    const element = panel.current;
    return () => {
      if (element === null || !element.contains(document.activeElement)) {
        return;
      }
      const page = element.closest<HTMLElement>("[data-rows-page]");
      const others = Array.from(page?.querySelectorAll<HTMLElement>("[data-rows-panel] [data-row-title]") ?? []).filter((title) => !element.contains(title));
      (others[0] ?? page)?.focus();
    };
  }, []);
  // `rows` are the ones drawn, in order, so a row's place in the panel is its place in the list.
  const onFocusCapture = (event: FocusEvent<HTMLElement>) => {
    const item = event.target instanceof Element ? event.target.closest<HTMLElement>("li[data-row]") : null;
    const items = Array.from(panel.current?.querySelectorAll<HTMLElement>("li[data-row]") ?? []);
    const at = item === null ? -1 : items.indexOf(item);
    const row = rows[at];
    if (row !== undefined) {
      focused.current = { key: row.key, at };
    }
  };
  return { panel, onFocusCapture };
}

interface PanelProps {
  /** The panel's name, in its head: a string, or a link to the thing the rows belong to. None: the rows alone, for a panel something else (a tab) already names. */
  title?: ReactNode;
  /** The head's name for assistive technology when `title` is not a string. */
  label?: string;
  /** Set at the head's right end, beside the count: a short figure in mono. */
  aside?: ReactNode;
  testId: string;
  rows: PanelRow[];
  /** What acts on the whole list, at its foot beside Show all. */
  foot?: ReactNode;
  /** Shown as a fold with its count, open when `open` (and on paper). */
  folded?: boolean;
  open?: boolean;
  /** Finished work is shown at full strength (a list of what is wrong is not faded for being done). */
  bright?: boolean;
}

/**
 * A list of rows in a panel: its first `SHOWN_MAX` rows, and "Show all" for the rest. Its head
 * says what it is and how many it holds. The page it sits in carries `data-rows-page` (and is
 * focusable), so a panel that goes away hands its focus to the page.
 */
export function RowsPanel({ title, label, aside, testId, rows, foot, folded = false, open = false, bright = false }: PanelProps) {
  const [all, setAll] = useState(false);
  const printing = usePrinting();
  const { panel, onFocusCapture } = useKeptFocus(all ? rows : rows.slice(0, SHOWN_MAX));
  const fold = useRef<HTMLDetailsElement>(null);
  // A fold the viewer closed is opened for paper: React will not reapply an attribute it thinks is set.
  useLayoutEffect(() => {
    if (printing && fold.current !== null) {
      fold.current.open = true;
    }
  }, [printing]);
  const toggle =
    rows.length <= SHOWN_MAX ? undefined : (
      <Button ghost className="no-print" data-testid="rows-show-all" aria-expanded={all} onClick={() => { setAll(!all); }}>
        {all ? "Show fewer" : `Show all ${String(rows.length)}`}
      </Button>
    );
  const bottom =
    toggle === undefined && foot === undefined ? undefined : (
      <span className="row rows-foot">
        {toggle}
        {foot}
      </span>
    );
  const listed = (
    <NodePanel {...(folded ? {} : { title, label, count: rows.length, aside, testId })} foot={bottom}>
      {rows.map((row, at) => {
        const shown = at < SHOWN_MAX || all;
        return shown || printing ? <Fragment key={row.key}>{row.render(shown)}</Fragment> : null;
      })}
    </NodePanel>
  );
  if (folded) {
    return (
      <details ref={fold} className="rows-fold" open={open || printing} data-testid={testId} data-count={rows.length}>
        <summary className="node-panel-head">
          {title} <span className="fold-count">{rows.length}</span>
        </summary>
        {listed}
      </details>
    );
  }
  return (
    <div ref={panel} className={bright ? "rows-panel rows-bright" : "rows-panel"} tabIndex={-1} data-rows-panel data-count={rows.length} onFocusCapture={onFocusCapture}>
      {listed}
    </div>
  );
}
