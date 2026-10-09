// C18: the journey status summary, for observers and reporting: the key figures, the in-scope
// nodes by state, what is overdue, short, or stale and why, the upcoming milestones with their
// effective dates, and the open decisions with their owners. A printable page: printing it
// leaves out the navigation, the controls, and the inspector (ui/app.css). Each node
// opens its detail (5.1) beside the summary.
import type { ReactNode } from "react";

import type { Ready } from "../detail/model.ts";
import { NodeLink } from "../detail/parts.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import type { SummaryModel } from "./model.ts";
import "./summary.css";

const ORIGIN_WORDS = { actual: "actual", pin: "pinned", due: "derived due" } as const;

function Figure({ label, count, testId, tone = "plain" }: { label: string; count: number; testId: string; tone?: "plain" | "bad" | "warn" }) {
  return (
    <div className={`summary-figure summary-figure-${count > 0 ? tone : "plain"}`} data-testid={testId} data-count={count}>
      <span className="summary-figure-count">{count}</span>
      <span className="muted small">{label}</span>
    </div>
  );
}

function Part({ title, testId, count, children }: { title: string; testId: string; count: number; children: ReactNode }) {
  return (
    <section className="panel stack summary-part" aria-label={title} data-testid={testId} data-count={count}>
      <h2>{title}</h2>
      {count === 0 ? <span className="muted small">None.</span> : children}
    </section>
  );
}

function Figures({ model }: { model: SummaryModel }) {
  return (
    <div className="summary-figures" data-testid="summary-figures">
      <Figure label="in scope" count={model.inScope} testId="summary-in-scope" />
      <Figure label="remaining" count={model.remaining} testId="summary-remaining" />
      <Figure label="overdue" count={model.overdue.length} testId="summary-overdue-count" tone="bad" />
      <Figure label="short" count={model.shortfalls.length} testId="summary-shortfall-count" tone="bad" />
      <Figure label="stale" count={model.stale.length} testId="summary-stale-count" tone="warn" />
      <Figure label="open decisions" count={model.openDecisions.length} testId="summary-open-count" tone="warn" />
    </div>
  );
}

function ByState({ model }: { model: SummaryModel }) {
  return (
    <Part title="In scope, by state" testId="summary-by-state" count={model.inScope}>
      <table className="data">
        <tbody>
          {model.byState.map((each) => (
            <tr key={each.state} data-testid="summary-state" data-state={each.state} data-count={each.count}>
              <td>{each.words}</td>
              <td className="summary-number">{each.count}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Part>
  );
}

function Trouble({ ready, model, selected }: { ready: Ready; model: SummaryModel; selected: string | undefined }) {
  return (
    <>
      <Part title="Overdue" testId="summary-overdue" count={model.overdue.length}>
        <ul className="detail-list">
          {model.overdue.map((each) => (
            <li key={each.key} data-testid="summary-item" data-node={each.key} data-selected={each.key === selected}>
              <NodeLink view={ready} node={each.key} />{" "}
              <span className="muted small">
                due {each.due ?? "?"}
                {each.lateDays === undefined ? "" : `, ${String(each.lateDays)} days late`}
              </span>
            </li>
          ))}
        </ul>
      </Part>
      <Part title="Short of days" testId="summary-shortfalls" count={model.shortfalls.length}>
        <ul className="detail-list">
          {model.shortfalls.map((each) => (
            <li key={each.key} data-testid="summary-item" data-node={each.key} data-selected={each.key === selected}>
              <NodeLink view={ready} node={each.key} /> <Badge tone="bad">{each.days ?? "?"} days short</Badge>
            </li>
          ))}
        </ul>
      </Part>
      <Part title="Stale" testId="summary-stale" count={model.stale.length}>
        <ul className="detail-list">
          {model.stale.map((each) => (
            <li key={each.key} data-testid="summary-item" data-node={each.key} data-selected={each.key === selected}>
              <NodeLink view={ready} node={each.key} /> <span className="muted small">{each.reasons.join("; ")}</span>
            </li>
          ))}
        </ul>
      </Part>
    </>
  );
}

function Ahead({ ready, model, selected }: { ready: Ready; model: SummaryModel; selected: string | undefined }) {
  return (
    <>
      <Part title="Upcoming milestones" testId="summary-upcoming" count={model.upcoming.length}>
        <table className="data">
          <thead>
            <tr>
              <th>Milestone</th>
              <th>Date</th>
              <th>From</th>
              <th>Owner</th>
            </tr>
          </thead>
          <tbody>
            {model.upcoming.map((each) => (
              <tr key={each.key} data-testid="summary-item" data-node={each.key} data-date={each.date} data-selected={each.key === selected}>
                <td><NodeLink view={ready} node={each.key} /></td>
                <td className="summary-date">{each.date}</td>
                <td><Badge>{ORIGIN_WORDS[each.origin]}</Badge></td>
                <td>{each.owners.length === 0 ? <span className="muted small">unassigned</span> : each.owners.join(", ")}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Part>
      <Part title="Open decisions" testId="summary-open" count={model.openDecisions.length}>
        <table className="data">
          <thead>
            <tr>
              <th>Decision, by rank</th>
              <th>Owner</th>
            </tr>
          </thead>
          <tbody>
            {model.openDecisions.map((each) => (
              <tr key={each.key} data-testid="summary-item" data-node={each.key} data-selected={each.key === selected}>
                <td><NodeLink view={ready} node={each.key} /></td>
                <td>{each.owners.length === 0 ? <span className="muted small">unassigned</span> : each.owners.join(", ")}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Part>
    </>
  );
}

/** C18: the status summary of a projected journey. */
export function StatusSummaryView({ ready, model, selected }: { ready: Ready; model: SummaryModel; selected: string | undefined }) {
  return (
    <div className="stack summary" data-testid="summary">
      <div className="row">
        <h2>Status summary</h2>
        <span className="muted small">as of {ready.derived.today}</span>
        <span className="spacer" />
        <Button className="no-print" onClick={() => { globalThis.print(); }}>
          Print
        </Button>
      </div>
      <Figures model={model} />
      <div className="summary-parts">
        <Ahead ready={ready} model={model} selected={selected} />
        <Trouble ready={ready} model={model} selected={selected} />
        <ByState model={model} />
      </div>
    </div>
  );
}
