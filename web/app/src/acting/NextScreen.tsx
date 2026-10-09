// C10: the next list, "what do I do now". The acting frontier in rank order (re-sortable by
// any single signal, or ranked for the viewer: prioritize for me), each item with its
// breadcrumb, why it ranks where it does, and its kind's actions inline (C11), an unassigned
// one with "assign owner" (D2). Filterable to only mine and by kind. When nothing can be acted
// on, the stalled diagnostic with unsnooze (D5). The engine's `next` projection answers it from
// the tab's derivation; what it shows lives in the address.
import { useMemo } from "react";
import { useLocation, useNavigate, useParams } from "react-router";

import { useProjected } from "../canvas/hooks.ts";
import type { Ready } from "../detail/model.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge } from "../ui/kit.tsx";
import { factsOf } from "./acts.ts";
import { Acts } from "./Acts.tsx";
import { ACTING_KINDS, nextFrom, nextPath, nextQueryOf, type NextSettings } from "./address.ts";
import { Check, Checks, SortSelect } from "./Controls.tsx";
import { ActingFrame } from "./Frame.tsx";
import { Crumb, DetailLink, Flags, Why } from "./Parts.tsx";
import { StalledPanel } from "./Stalled.tsx";
import type { NodeRow } from "./why.ts";

function NextControls({ settings, onChange }: { settings: NextSettings; onChange: (next: NextSettings) => void }) {
  return (
    <div className="row acting-controls" data-testid="next-controls">
      <SortSelect sort={settings.sort} onChange={(sort) => { onChange({ ...settings, sort }); }} />
      <Check label="Prioritize for me" checked={settings.forMe} testId="for-me" onChange={(forMe) => { onChange({ ...settings, forMe }); }} />
      <Check label="Only mine" checked={settings.mine} testId="only-mine" onChange={(mine) => { onChange({ ...settings, mine }); }} />
      <Checks legend="Kinds" options={ACTING_KINDS} chosen={settings.kinds} words={(kind) => kind} testId="kind" onChange={(kinds) => { onChange({ ...settings, kinds }); }} />
    </div>
  );
}

function Item({ view, row, at, settings }: { view: Ready; row: NodeRow; at: number; settings: NextSettings }) {
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
        <span className="muted">
          {row.due == null ? "no deadline" : `due ${row.due}`}
          {row.slack_days == null ? "" : `, slack ${String(row.slack_days)} days`}
        </span>
      </div>
      <Why view={view} row={row} sort={settings.sort} />
      {facts === undefined ? null : <Acts view={view} facts={facts} />}
    </li>
  );
}

function NextBody({ view, settings }: { view: Ready; settings: NextSettings }) {
  const navigate = useNavigate();
  const journey = view.journey.header.id;
  const request = useMemo(() => ({ projection: "next" as const, query: nextQueryOf(settings) }), [settings]);
  const { value: next, error } = useProjected(view, request);
  return (
    <section className="stack" aria-label="Next" data-testid="next">
      <NextControls settings={settings} onChange={(changed) => void navigate(nextPath(journey, changed))} />
      {error === undefined ? null : <p className="callout callout-bad">The next list could not be read: {error}</p>}
      {next === undefined ? <p className="muted">Ranking the frontier...</p> : null}
      {next !== undefined && next.items.length === 0 ? (
        next.stalled == null ? (
          <p className="callout" data-testid="next-empty">
            {settings.mine || settings.kinds.length > 0 ? "Nothing on the acting frontier matches these filters." : "Nothing is left to act on in this journey."}
          </p>
        ) : (
          <StalledPanel view={view} />
        )
      ) : null}
      <ol className="stack next-list">
        {(next?.items ?? []).map((row, at) => (
          <Item key={row.key} view={view} row={row} at={at} settings={settings} />
        ))}
      </ol>
    </section>
  );
}

export function NextScreen() {
  const { id = "" } = useParams();
  const { search } = useLocation();
  const settings = useMemo(() => nextFrom(new URLSearchParams(search)), [search]);
  return (
    <ActingFrame key={id} id={id} screen="next">
      {(view) => <NextBody view={view} settings={settings} />}
    </ActingFrame>
  );
}
