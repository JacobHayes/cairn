// C10: the next list, "what do I do now". One list of the acting frontier in the shared rank
// (sortable by one signal, or ranked for the viewer), each row saying one thing in words, its
// primary action at the right end, and the rest in the inspector a click away. Mine narrows it
// to the viewer's, and kinds and flags narrow it further. Under it, closed: what needs a look
// and what is snoozed. When nothing can be acted on, the stalled diagnostic with unsnooze (D5).
// The engine's `next` projection answers it from the tab's derivation; what it shows lives in
// the address. It is NEXT, LIST: the journey page (screens/JourneyFrame.tsx) holds its toolbar
// and the filters it opens (`NextControls`).
import { useMemo } from "react";
import { useLocation } from "react-router";

import { useProjected } from "../canvas/hooks.ts";
import { titleOf, type Ready } from "../detail/model.ts";
import { nodePath, screenPath } from "../detail/parts.tsx";
import { useMineOf } from "../journeys/mine.ts";
import { ACTING_KINDS, NEXT_FILTER_FLAGS, nextQueryOf, type NextSettings } from "./address.ts";
import { Check, Checks, FlagChecks, SortSelect } from "./Controls.tsx";
import { Folds } from "./Folds.tsx";
import { NextForYou } from "./NextForYou.tsx";
import { NextRow } from "./NextRow.tsx";
import { withFlags } from "./rows.ts";
import { StalledPanel } from "./Stalled.tsx";
import type { NodeRow } from "./why.ts";
import "./acting.css";

/** How many rows wear a rank tag. */
const TAGGED = 3;

/** C10: mine, kinds and flags: what the filter holds on NEXT, LIST (the sort, with "Rank for me", is in the list's header). */
export function NextControls({ settings, onChange }: { settings: NextSettings; onChange: (next: NextSettings) => void }) {
  return (
    <div className="stack acting-controls" data-testid="next-controls">
      <Check label="Only mine" checked={settings.mine} testId="only-mine" onChange={(mine) => { onChange({ ...settings, mine }); }} />
      <Checks legend="Kinds" options={ACTING_KINDS} chosen={settings.kinds} words={(kind) => kind} testId="kind" stacked onChange={(kinds) => { onChange({ ...settings, kinds }); }} />
      <FlagChecks options={NEXT_FILTER_FLAGS} chosen={settings.flags} onChange={(flags) => { onChange({ ...settings, flags }); }} />
    </div>
  );
}

/** Whether `row`'s title or its place in the journey holds the search `text`. */
function matches(view: Ready, row: NodeRow, text: string): boolean {
  const wanted = text.trim().toLowerCase();
  return wanted === "" || [row.title, ...(row.ancestors ?? []).map((key) => titleOf(view, key))].some((each) => each.toLowerCase().includes(wanted));
}

/** The keys of the rows with the highest ranks, best first: the ones that wear a tag, wherever the sort puts them. */
function topRanked(rows: readonly NodeRow[]): string[] {
  return rows
    .filter((row) => row.rank != null)
    .map((row, at) => ({ key: row.key, rank: row.rank?.rank ?? 0, at }))
    .sort((left, right) => right.rank - left.rank || left.at - right.at)
    .slice(0, TAGGED)
    .map((each) => each.key);
}

const things = (count: number) => `${String(count)} ${count === 1 ? "thing" : "things"} to do now`;

/** NEXT, LIST: the acting frontier in rank order, narrowed by what the toolbar and the filter hold. */
export function NextList({ view, settings, selected, onSettings }: { view: Ready; settings: NextSettings; selected: string | undefined; onSettings: (next: NextSettings) => void }) {
  const { pathname, search } = useLocation();
  const request = useMemo(() => ({ projection: "next" as const, query: nextQueryOf(settings) }), [settings]);
  const { value: next, error } = useProjected(view, request);
  const mine = useMineOf(view.journey.header.id);
  const items = withFlags(next?.items ?? [], settings.flags).filter((row) => matches(view, row, settings.text));
  const tagged = topRanked(items);
  // The stalled panel already says what the journey waits on, so the line that says the same is left out.
  const stalled = next !== undefined && items.length === 0 && next.stalled != null && next.items.length === 0;
  const toOf = (key: string) => `${nodePath(screenPath(pathname), key)}${search}`;
  return (
    <section className="stack next" aria-label="Next" data-testid="next">
      {mine.status === "ready" && !stalled ? <NextForYou view={view} entries={mine.entries} /> : null}
      {next !== undefined && items.length === 0 ? null : (
        <div className="row next-head">
          <span className="muted" data-testid="next-count">{next === undefined ? "Ranking the frontier..." : things(items.length)}</span>
          <span className="spacer" />
          <SortSelect sort={settings.sort} forMe={settings.forMe} onChange={(sort, forMe) => { onSettings({ ...settings, sort, forMe }); }} />
        </div>
      )}
      {error === undefined ? null : <p className="callout callout-bad">The next list could not be read: {error}</p>}
      {next !== undefined && items.length === 0 ? (
        !stalled ? (
          <p className="callout" data-testid="next-empty">
            {settings.mine || settings.kinds.length > 0 || settings.flags.length > 0 || settings.decisions || settings.text !== ""
              ? "Nothing on the acting frontier matches these filters."
              : "Nothing is left to act on in this journey."}
          </p>
        ) : (
          <StalledPanel view={view} />
        )
      ) : null}
      <ol className="plain-list next-list">
        {items.map((row) => (
          <NextRow key={row.key} view={view} row={row} to={toOf(row.key)} selected={row.key === selected} sort={settings.sort} tag={tagged.includes(row.key) ? tagged.indexOf(row.key) + 1 : undefined} />
        ))}
      </ol>
      <Folds view={view} settings={settings} toOf={toOf} />
    </section>
  );
}
