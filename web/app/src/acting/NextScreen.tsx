// C10: the next list, "what do I do now". The acting frontier in rank order (re-sortable by
// any single signal, or ranked for the viewer: prioritize for me), each item with its
// breadcrumb, why it ranks where it does, and its kind's actions inline (C11), an unassigned
// one with "assign owner" (D2). Filterable to only mine and by kind. When nothing can be acted
// on, the stalled diagnostic with unsnooze (D5). The engine's `next` projection answers it from
// the tab's derivation; what it shows lives in the address. It is NEXT, LIST: the journey page
// (screens/JourneyFrame.tsx) holds its toolbar and the filters it opens (`NextControls`).
import { useMemo } from "react";

import { useProjected } from "../canvas/hooks.ts";
import { titleOf, type Ready } from "../detail/model.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge } from "../ui/kit.tsx";
import { factsOf } from "./acts.ts";
import { Acts } from "./Acts.tsx";
import { ACTING_KINDS, nextQueryOf, type NextSettings } from "./address.ts";
import { Check, Checks, SortSelect } from "./Controls.tsx";
import { Crumb, DetailLink, Flags, Why } from "./Parts.tsx";
import { StalledPanel } from "./Stalled.tsx";
import type { NodeRow } from "./why.ts";
import "./acting.css";

/** C10: the ranking for the viewer, mine, and kinds: what the filter holds on NEXT, LIST (the sort is in the list's header). */
export function NextControls({ settings, onChange }: { settings: NextSettings; onChange: (next: NextSettings) => void }) {
  return (
    <div className="stack acting-controls" data-testid="next-controls">
      <Check label="Prioritize for me" checked={settings.forMe} testId="for-me" onChange={(forMe) => { onChange({ ...settings, forMe }); }} />
      <Check label="Only mine" checked={settings.mine} testId="only-mine" onChange={(mine) => { onChange({ ...settings, mine }); }} />
      <Checks legend="Kinds" options={ACTING_KINDS} chosen={settings.kinds} words={(kind) => kind} testId="kind" stacked onChange={(kinds) => { onChange({ ...settings, kinds }); }} />
    </div>
  );
}

function Item({ view, row, at, settings, inspected }: { view: Ready; row: NodeRow; at: number; settings: NextSettings; inspected: boolean }) {
  const facts = factsOf(view, row.key);
  return (
    <li className="panel stack next-item" data-testid="next-item" data-node={row.key} data-slack={row.slack_days ?? ""} data-gravity={row.gravity} data-leverage={row.leverage}>
      <Crumb view={view} row={row} />
      <div className="row">
        <span className="next-position">{at + 1}</span>
        <DetailLink view={view} node={row.key} className="next-title" />
        <Badge>{row.kind}</Badge>
        <Badge tone={statusTone(row.display_state)}>{statusWord(row.display_state, row.kind)}</Badge>
        <Flags view={view} row={row} />
        <span className="muted small">
          {row.due == null ? "no deadline" : `due ${row.due}`}
          {row.slack_days == null ? "" : `, slack ${String(row.slack_days)} days`}
        </span>
      </div>
      <Why view={view} row={row} sort={settings.sort} />
      {facts === undefined || inspected ? null : <Acts view={view} facts={facts} />}
    </li>
  );
}

/** Whether `row`'s title or its place in the journey holds the search `text`. */
function matches(view: Ready, row: NodeRow, text: string): boolean {
  const wanted = text.trim().toLowerCase();
  return wanted === "" || [row.title, ...(row.ancestors ?? []).map((key) => titleOf(view, key))].some((each) => each.toLowerCase().includes(wanted));
}

/** NEXT, LIST: the acting frontier in rank order, narrowed by what the toolbar and the filter hold. */
export function NextList({ view, settings, selected, onSettings }: { view: Ready; settings: NextSettings; selected: string | undefined; onSettings: (next: NextSettings) => void }) {
  const request = useMemo(() => ({ projection: "next" as const, query: nextQueryOf(settings) }), [settings]);
  const { value: next, error } = useProjected(view, request);
  const items = (next?.items ?? []).filter((row) => matches(view, row, settings.text));
  return (
    <section className="stack" aria-label="Next" data-testid="next">
      <div className="row acting-pass-controls">
        <SortSelect sort={settings.sort} onChange={(sort) => { onSettings({ ...settings, sort }); }} />
      </div>
      {error === undefined ? null : <p className="callout callout-bad">The next list could not be read: {error}</p>}
      {next === undefined ? <p className="muted small">Ranking the frontier...</p> : null}
      {next !== undefined && items.length === 0 ? (
        next.stalled == null || next.items.length > 0 ? (
          <p className="callout" data-testid="next-empty">
            {settings.mine || settings.kinds.length > 0 || settings.decisions || settings.text !== ""
              ? "Nothing on the acting frontier matches these filters."
              : "Nothing is left to act on in this journey."}
          </p>
        ) : (
          <StalledPanel view={view} />
        )
      ) : null}
      <ol className="stack next-list">
        {items.map((row, at) => (
          <Item key={row.key} view={view} row={row} at={at} settings={settings} inspected={row.key === selected} />
        ))}
      </ol>
    </section>
  );
}
