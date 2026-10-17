// The shared list of the Summary: a panel of node rows, the first five of them and "Show all" for
// the rest, a fold of the same, and the keeping of the focus as rows move away (C18). On paper
// every row is there and no control is.
import { useEffect, useLayoutEffect, useRef, useState, type FocusEvent, type ReactNode } from "react";
import { flushSync } from "react-dom";

import { NodeItem, NodePanel } from "../acting/NodeItem.tsx";
import { RowActions } from "../acting/RowActions.tsx";
import type { NodeRow } from "../acting/why.ts";
import type { Ready } from "../detail/model.ts";
import { Button } from "../ui/kit.tsx";
import { nodeAddress } from "./links.ts";
import type { FactTone } from "./model.ts";

/** How many rows a list shows before "Show all": the rest are on paper, and one click away. */
export const SHOWN_MAX = 5;

/** The owner as a row says it when it offers no actions: their names, or that nobody has it. */
function OwnerText({ names }: { names: string[] | undefined }) {
  if (names === undefined) {
    return null;
  }
  return (
    <div className="row-side">
      <span className="row-owner row-owner-static" title={names.length === 0 ? undefined : `Owned by ${names.join(", ")}`}>
        {names.length === 0 ? "Unassigned" : names.join(", ")}
      </span>
    </div>
  );
}

/** One row of a list: a node with its one fact, and what it offers when `live`. */
export interface Entry {
  key: string;
  /** What it says under the title; a function is read only when the row is drawn (a long list reads five). */
  fact: ReactNode | (() => ReactNode);
  tone: FactTone | undefined;
  /** The row's actions, for a row that has them; read-only rows give its owner's names. */
  act: NodeRow | undefined;
  /** The owners' names, none when the row says nothing of who has it (finished work). */
  owners: string[] | undefined;
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
 * passed on) the row now in its place takes it, or the panel when the list empties. Nothing
 * else moves the focus, so a person who went elsewhere is not pulled back.
 */
function useKeptFocus(entries: Entry[]) {
  const panel = useRef<HTMLDivElement>(null);
  const focused = useRef<{ key: string; at: number } | undefined>(undefined);
  const keys = entries.map((entry) => entry.key).join("\n");
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
    const titles = panel.current?.querySelectorAll<HTMLElement>(".node-title");
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
      const page = element.closest<HTMLElement>(".summary");
      const others = Array.from(page?.querySelectorAll<HTMLElement>(".summary-panel .node-title") ?? []).filter((title) => !element.contains(title));
      (others[0] ?? page)?.focus();
    };
  }, []);
  const onFocusCapture = (event: FocusEvent<HTMLElement>) => {
    const item = event.target instanceof Element ? event.target.closest<HTMLElement>("li[data-node]") : null;
    const key = item?.dataset["node"];
    if (key !== undefined) {
      focused.current = { key, at: entries.findIndex((entry) => entry.key === key) };
    }
  };
  return { panel, onFocusCapture };
}

/**
 * The rows of a list: the first `SHOWN_MAX`, then the rest once `all` is asked for. On paper the
 * rest are there too, read-only, so a long journey draws five rows a list and not its whole size.
 */
function Rows({ ready, entries, actions, owner, all }: { ready: Ready; entries: Entry[]; actions: boolean; owner: boolean; all: boolean }) {
  const journey = ready.journey.header.id;
  const printing = usePrinting();
  return entries.map((entry, at) => {
    const revealed = at < SHOWN_MAX || all;
    if (!revealed && !printing) {
      return null;
    }
    const to = nodeAddress(journey, entry.key);
    return (
      <NodeItem key={entry.key} view={ready} node={entry.key} to={to} fact={typeof entry.fact === "function" ? entry.fact() : entry.fact} factTone={entry.tone} testId="summary-item">
        {actions && entry.act !== undefined && revealed ? <RowActions view={ready} row={entry.act} to={to} owner={owner} /> : <OwnerText names={entry.owners} />}
      </NodeItem>
    );
  });
}

interface PanelProps {
  ready: Ready;
  title: string;
  testId: string;
  entries: Entry[];
  /** Whether rows that can act offer their actions (not on an archived journey). */
  actions: boolean;
  owner?: boolean;
  /** What acts on the whole list, at its foot beside Show all. */
  foot?: ReactNode;
  /** Shown as a fold with its count, open when `open` (and on paper). */
  folded?: boolean;
  open?: boolean;
  /** Finished work is shown at full strength (a list of what is wrong is not faded for being done). */
  bright?: boolean;
}

/**
 * A list of rows in a panel: its first `SHOWN_MAX` rows, and "Show all" for the rest, whose
 * rows offer their actions once shown. Its head says what it is and how many it holds.
 */
export function RowsPanel({ ready, title, testId, entries, actions, owner = true, foot, folded = false, open = false, bright = false }: PanelProps) {
  const [all, setAll] = useState(false);
  const printing = usePrinting();
  const { panel, onFocusCapture } = useKeptFocus(entries);
  const fold = useRef<HTMLDetailsElement>(null);
  // A fold the viewer closed is opened for paper: React will not reapply an attribute it thinks is set.
  useLayoutEffect(() => {
    if (printing && fold.current !== null) {
      fold.current.open = true;
    }
  }, [printing]);
  const toggle =
    entries.length <= SHOWN_MAX ? undefined : (
      <Button ghost className="no-print" data-testid="summary-show-all" aria-expanded={all} onClick={() => { setAll(!all); }}>
        {all ? "Show fewer" : `Show all ${String(entries.length)}`}
      </Button>
    );
  const bottom =
    toggle === undefined && foot === undefined ? undefined : (
      <span className="row summary-foot">
        {toggle}
        {foot}
      </span>
    );
  const listed = (
    <NodePanel {...(folded ? {} : { title, count: entries.length, testId })} foot={bottom}>
      <Rows ready={ready} entries={entries} actions={actions} owner={owner} all={all} />
    </NodePanel>
  );
  if (folded) {
    return (
      <details ref={fold} className="summary-fold" open={open || printing} data-testid={testId} data-count={entries.length}>
        <summary className="node-panel-head">
          {title} <span className="fold-count">{entries.length}</span>
        </summary>
        {listed}
      </details>
    );
  }
  return (
    <div ref={panel} className={bright ? "summary-panel summary-bright" : "summary-panel"} tabIndex={-1} data-count={entries.length} onFocusCapture={onFocusCapture}>
      {listed}
    </div>
  );
}
