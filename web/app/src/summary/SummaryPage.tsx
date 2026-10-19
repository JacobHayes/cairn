// C18: the Summary, the journey's front page: where it stands (what is left, the counts by state,
// the final milestone), what is wrong, what to do (what waits on the viewer, the decisions to
// make now, the rest of what is next, the decisions still waiting), and the context (milestones,
// what finished lately, the description, notes and lineage). Every value is one the journey's
// local derivation already gives (model.ts), kept current as it moves (H6). The lead and the
// sections are read in that order at every width; a wide page only places the last three in a
// side column. Printed, every list is whole and the page is dated (it is the report, C18).
import type { ReactNode } from "react";
import { Link } from "react-router";

import { NodePanel } from "../acting/NodeItem.tsx";
import { NextForYou } from "../acting/NextForYou.tsx";
import { factOf } from "../acting/reasons.ts";
import { StalledPanel } from "../acting/Stalled.tsx";
import type { NodeRow } from "../acting/why.ts";
import { nextForYou } from "../acting/yours.ts";
import { useProjected } from "../canvas/hooks.ts";
import { errorWords, valueOf } from "../data/keyed-read.ts";
import { useViewer } from "../data/react.ts";
import { AnnotationList } from "../detail/Attachments.tsx";
import type { Ready } from "../detail/model.ts";
import { LineageBlock, Flags, useLineage } from "../journeys/JourneyFacts.tsx";
import { progressOf } from "../journeys/figures-model.ts";
import { useMineOf } from "../journeys/mine.ts";
import { useAssistantAsk } from "../assistant/ask.ts";
import { isSoon } from "../status/when.ts";
import { dateWords, dayOf } from "../timeline/model.ts";
import { Button, Meter } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { nodeAddress, planAddress, timelineAddress } from "./links.ts";
import {
  attentionRows,
  blockedRows,
  claimActions,
  claimedKeys,
  isEmpty,
  lifecycleLine,
  milestoneLine,
  ORIGIN_WORDS,
  ownersOf,
  recentlyFinished,
  shortfallWords,
  summaryModel,
  underWay,
  whenWords,
  type MilestoneLine,
  type SummaryModel,
} from "./model.ts";
import { EntriesPanel, type Entry } from "./Entries.tsx";
import "./summary.css";
import { Failed } from "../ui/Failed.tsx";

/** The date of a milestone line in words, where it comes from on hover, and its own shortfall as a sentence of its own. */
function MilestoneWhen({ ready, line }: { ready: Ready; line: MilestoneLine }) {
  const today = ready.derived.today;
  const journey = ready.journey.header.id;
  if (line.date === undefined) {
    return (
      <>
        no date yet
        {line.feeder === undefined ? null : (
          <>
            , set by <Link to={nodeAddress(journey, line.feeder.key)}>{line.feeder.title}</Link>
          </>
        )}
        .
      </>
    );
  }
  // F1: an auto-reaching milestone whose date has passed is reached, whatever its date came from.
  const reached = line.reached;
  const late = !reached && dayOf(line.date) < dayOf(today);
  const soon = !reached && isSoon(line.date, today);
  const origin = line.origin === undefined ? undefined : ORIGIN_WORDS[line.origin];
  return (
    <>
      <span className={late ? "flag-bad" : soon ? "flag-warn" : undefined} title={origin === undefined ? undefined : `This date is ${origin}`}>
        {whenWords(line.date, reached ? "actual" : line.origin, today)}
      </span>
      .
      {line.shortfallDays === undefined ? null : <span className="flag-bad">{` ${shortfallWords(ready, line.key, line.shortfallDays)}.`}</span>}
    </>
  );
}

/** The lead: what is left, its counts, the milestone, and what is wrong, in one panel. */
function Lead({ ready, model, note }: { ready: Ready; model: SummaryModel; note: ReactNode }) {
  const journey = ready.journey.header.id;
  const { done, inScope } = progressOf(model);
  const line = milestoneLine(ready, model);
  const stalled = ready.derived.stalled != null;
  const lifecycle = lifecycleLine(ready, model);
  const active = ready.journey.header.status === "active";
  return (
    <section className="panel stack summary-lead" aria-label="Where it stands" data-testid="summary-lead">
      <div className="summary-lead-head">
        {model.remaining === 0 ? (
          <span className="summary-left" data-testid="summary-left" data-remaining={0} data-in-scope={inScope}>
            Nothing left in scope
          </span>
        ) : (
          <span className="summary-left" data-testid="summary-left" data-remaining={model.remaining} data-in-scope={inScope}>
            <span className="summary-figure">{model.remaining}</span> {active ? "left" : "unfinished"} of {inScope} in scope
          </span>
        )}
        <span className="summary-asof">As of {dateWords(ready.derived.today, ready.derived.today)}</span>
      </div>
      {inScope === 0 ? null : (
        <Meter done={done} of={inScope} label={`${String(done)} of ${String(inScope)} in scope finished or skipped`} data-testid="summary-meter" data-done={done} />
      )}
      {model.byState.length === 0 ? null : (
        <p className="summary-states" data-testid="summary-states">
          {model.byState.map((each, at) => (
            <span key={each.state} data-state={each.state} data-count={each.count}>
              {at === 0 ? "" : " · "}
              <span className="summary-count">{each.count}</span> {each.words}
            </span>
          ))}
        </p>
      )}
      {lifecycle === undefined ? null : (
        <p className="summary-lifecycle" data-testid="summary-lifecycle">
          {lifecycle}
        </p>
      )}
      {line === undefined ? null : (
        <p className="summary-milestone" data-testid="summary-milestone" data-role={line.role} data-node={line.key}>
          {line.role === "final" ? (active ? "Final milestone" : "Planned final milestone") : "Next milestone"}: <Link to={nodeAddress(journey, line.key)}>{line.title}</Link>, <MilestoneWhen ready={ready} line={line} />
        </p>
      )}
      {stalled ? <StalledPanel view={ready} nodeTo={(node) => nodeAddress(journey, node)} readOnly={ready.journey.header.status === "archived"} endDate={line?.role === "final" && !line.reached ? line.date : undefined} /> : null}
      {note}
    </section>
  );
}

/**
 * The milestones ahead, earliest first, each one line: its title and when it falls, not another
 * full node row (the lead already names the end, and the rows above name what to act on). Its
 * own shortfall is said in words beside the date.
 */
function Dates({ ready, model, skip }: { ready: Ready; model: SummaryModel; skip: string | undefined }) {
  const today = ready.derived.today;
  const journey = ready.journey.header.id;
  const rows = model.upcoming.filter((each) => each.key !== skip);
  if (rows.length === 0) {
    return null;
  }
  return (
    <NodePanel title="Milestones" count={rows.length} testId="summary-milestones" foot={<Link to={timelineAddress(journey)}>Open the timeline</Link>}>
      {rows.map((each) => {
        const short = ready.derived.nodes[each.key]?.dates.shortfall?.shortfall_days;
        const origin = ORIGIN_WORDS[each.origin];
        return (
          <li key={each.key} className="summary-date" data-testid="summary-date" data-node={each.key}>
            <Link to={nodeAddress(journey, each.key)} className="node-title">
              {each.title}
            </Link>
            <span className={isSoon(each.date, today) ? "summary-date-when flag-warn" : "summary-date-when muted"} title={origin === undefined ? undefined : `This date is ${origin}`}>
              {whenWords(each.date, each.origin, today)}
            </span>
            {short === undefined ? null : <span className="flag-bad summary-date-short">{shortfallWords(ready, each.key, short)}</span>}
          </li>
        );
      })}
    </NodePanel>
  );
}

/** C18: what the journey is: its description, where it came from, its notes and links. */
function About({ ready, readOnly }: { ready: Ready; readOnly: boolean }) {
  const { header } = ready.journey;
  const lineage = useLineage(ready);
  const annotations = (ready.journey.graph.state?.annotations ?? []).filter((annotation) => annotation.body.node == null);
  return (
    <section className="panel stack summary-about" aria-label="About" data-testid="summary-about">
      <span className="label">About</span>
      {header.description == null ? null : <Markdown text={header.description} data-testid="overview-description" />}
      <LineageBlock ready={ready} lineage={lineage} readOnly={readOnly} />
      {readOnly && annotations.length === 0 ? null : <AnnotationList view={ready} node={null} annotations={annotations} bare readOnly={readOnly} showNone={false} />}
    </section>
  );
}

/** A journey with no node at all: the way to plan it (the graph in Edit mode, or the assistant). */
function Unplanned({ ready, readOnly }: { ready: Ready; readOnly: boolean }) {
  const ask = useAssistantAsk();
  return (
    <section className="panel stack summary-empty" aria-label="Nothing planned" data-testid="summary-empty">
      <span className="label">Plan this journey</span>
      <p className="summary-left">Nothing is planned here yet.</p>
      {readOnly ? null : (
        <span className="row">
          <Link className="button primary" to={planAddress(ready.journey.header.id)} data-testid="summary-plan">
            Plan it on the graph
          </Link>
          {ask === undefined ? null : (
            <>
              <span className="muted">or</span>
              <Button onClick={ask} data-testid="summary-ask">
                Ask the assistant
              </Button>
            </>
          )}
        </span>
      )}
    </section>
  );
}

/** The viewer's own work: their owned items, or the one line saying what is theirs next. */
function Yours({ ready, entries, actions }: { ready: Ready; entries: Entry[]; actions: boolean }) {
  const mine = useMineOf(ready.journey.header.id);
  const owned = mine.status === "ready" ? mine.entries.filter((entry) => entry.kinds.includes("k_owner")) : [];
  if (entries.length > 0) {
    return <EntriesPanel ready={ready} title="Waiting on you" testId="summary-yours" entries={entries} actions={actions} />;
  }
  return nextForYou(ready, owned) === undefined ? null : <NextForYou view={ready} entries={owned} nothing={false} />;
}

/** The rows each section lists, claimed so a node is in one action section only (model.ts `claimActions`). */
function entriesOf(ready: Ready, model: SummaryModel, frontier: NodeRow[], viewer: readonly string[]) {
  const claimed = claimActions(ready, model, frontier, ready.journey.header.status === "active" ? viewer : []);
  const line = milestoneLine(ready, model);
  const sets = line?.feeder;
  const rowEntry = (row: NodeRow): Entry => {
    const said = factOf(ready, row);
    const says = said?.tone === undefined && sets?.key === row.key ? `Sets the date of ${line?.title ?? "the final milestone"}` : said?.words;
    return { key: row.key, fact: says, tone: said?.tone, act: row, owners: ownersOf(ready, row.key) };
  };
  const plain = { tone: undefined, act: undefined } as const;
  const held = (each: { key: string; owners: string[]; fact: () => string | undefined }): Entry => ({ ...plain, key: each.key, fact: each.fact, owners: each.owners });
  const taken = claimedKeys(claimed);
  const attention = attentionRows(ready, model).map((each): Entry => {
    const chain = model.shortfalls.some((short) => short.key === each.key);
    const fact = (
      <>
        {each.reasons.join("; ")}
        {chain ? (
          <>
            {". "}
            <Link to={nodeAddress(ready.journey.header.id, each.key)} className="no-print">
              See the chain
            </Link>
          </>
        ) : null}
      </>
    );
    return { key: each.key, fact, tone: each.tone, act: undefined, owners: ownersOf(ready, each.key) };
  });
  return {
    yours: claimed.yours.map(rowEntry),
    decide: claimed.decide.map(rowEntry),
    nextUp: claimed.nextUp.map(rowEntry),
    underWay: underWay(ready, taken).map(held),
    waiting: claimed.waiting.map((each): Entry => ({ ...plain, key: each.key, fact: each.fact, owners: each.owners })),
    blocked: blockedRows(ready, taken).map(held),
    attention,
    recent: recentlyFinished(ready).map((each): Entry => ({ ...plain, key: each.key, fact: each.fact, owners: undefined })),
    milestoneSkip: line?.key,
  };
}

/** What the page is waiting for: the empty journey's invitation, or the words while it is read. */
function Waiting({ ready, error }: { ready: Ready; error: string | undefined }) {
  const status = ready.journey.header.status;
  if (isEmpty(ready)) {
    return (
      <div className="summary" data-testid="summary" data-status={status} tabIndex={-1}>
        <div className="summary-grid summary-grid-empty">
          <div className="summary-main stack">
            <Unplanned ready={ready} readOnly={status === "archived"} />
          </div>
          <div className="summary-side stack">
            <About ready={ready} readOnly={status === "archived"} />
          </div>
        </div>
      </div>
    );
  }
  return (
    <div className="summary stack" data-testid="summary" data-status={status} tabIndex={-1}>
      {error === undefined ? <p className="muted small">Summing up the journey...</p> : <p className="callout callout-bad"><Failed what="The summary" error={error} /></p>}
    </div>
  );
}

type Entries = ReturnType<typeof entriesOf>;

/** The action sections, in reading order: what waits on the viewer, the decisions to make, the rest of what is next, the work under way, then what waits (folded). */
function Actions({ ready, entries, model }: { ready: Ready; entries: Entries; model: SummaryModel }) {
  const status = ready.journey.header.status;
  const actions = status !== "archived";
  // A completed or archived journey lists what it left, not what to do next.
  const closed = status !== "active";
  const open = entries.yours.length + entries.decide.length + entries.nextUp.length === 0;
  const panel = (title: string, testId: string, rows: Entry[], more: { acts?: boolean; fold?: boolean } = {}) =>
    rows.length === 0 ? null : <EntriesPanel ready={ready} title={title} testId={testId} entries={rows} actions={more.acts === true && actions} folded={more.fold === true} open={open} />;
  return (
    <div className="summary-main stack">
      {status === "active" ? <Yours ready={ready} entries={entries.yours} actions={actions} /> : null}
      {panel(closed ? "Open decisions" : "Decide now", "summary-decide", entries.decide, { acts: true })}
      {panel(closed ? "Ready or active" : "Next up", "summary-next", entries.nextUp, { acts: true })}
      {panel("In progress", "summary-underway", entries.underWay)}
      {panel("Decisions waiting", "summary-waiting", entries.waiting, { fold: true })}
      {panel("Blocked", "summary-blocked", entries.blocked, { fold: true })}
      {open && entries.waiting.length === 0 && entries.blocked.length === 0 && model.remaining === 0 ? <p className="muted summary-nothing" data-testid="summary-nothing">Nothing left to do.</p> : null}
    </div>
  );
}

/** The Summary page: the lead, what needs attention, the action sections and the context. `note` is said under the lead. */
export function SummaryPage({ ready, note }: { ready: Ready; note?: ReactNode }) {
  const read = useProjected(ready, { projection: "status_summary" });
  const next = useProjected(ready, { projection: "next" });
  const { viewer } = useViewer();
  const status = ready.journey.header.status;
  const summary = valueOf(read);
  const frontier = valueOf(next)?.items;
  if (isEmpty(ready) || summary === undefined || frontier === undefined) {
    return <Waiting ready={ready} error={errorWords(read) ?? errorWords(next)} />;
  }
  const model = summaryModel(ready, summary);
  const entries = entriesOf(ready, model, frontier, viewer?.entities ?? []);
  return (
    <div className="summary rows-page" data-testid="summary" data-status={status} data-rows-page tabIndex={-1}>
      <div className="summary-body stack">
        <Lead ready={ready} model={model} note={note} />
        {entries.attention.length === 0 ? null : (
          <EntriesPanel ready={ready} title="Needs attention" testId="summary-attention" entries={entries.attention} actions={false} bright foot={<NeedsLook ready={ready} model={model} />} />
        )}
        <div className="summary-grid">
          <Actions ready={ready} entries={entries} model={model} />
          <Context ready={ready} entries={entries} model={model} archived={status === "archived"} />
        </div>
      </div>
    </div>
  );
}

/** What needs a look, as links into the plan list: the Needs attention panel's foot. */
function NeedsLook({ ready, model }: { ready: Ready; model: SummaryModel }) {
  return (
    <span className="summary-flags" data-testid="summary-exceptions">
      <span className="muted">Open in the list: </span>
      <Flags ready={ready} model={model} toned />
    </span>
  );
}

/** The side column on a wide page: the milestones ahead, what finished lately, and what the journey is. */
function Context({ ready, entries, model, archived }: { ready: Ready; entries: Entries; model: SummaryModel; archived: boolean }) {
  return (
    <div className="summary-side stack">
      <Dates ready={ready} model={model} skip={entries.milestoneSkip} />
      {entries.recent.length === 0 ? null : <EntriesPanel ready={ready} title="Recently finished" testId="summary-recent" entries={entries.recent} actions={false} bright />}
      <About ready={ready} readOnly={archived} />
    </div>
  );
}
