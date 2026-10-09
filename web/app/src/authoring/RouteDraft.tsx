// A route draft in the frame (design 4.9): the one-line header with Publish as the primary and
// the draft's `⋯`, the draft card the inspector shows while no node is open (what blocks
// publishing, what is only advisory, Publish and Discard), what an import or a publish leaves
// under the header, and the List projection. A route has no state or dates, so its cards say
// the date rule in words and no status.
import type { Schema } from "@cairn/client";
import { useEffect, useState, type ReactNode } from "react";
import { Link } from "react-router";

import { AssistantDock } from "../assistant/AssistantPanel.tsx";
import type { Route } from "../data/host.ts";
import { ImportFile, type ImportedHandler } from "../routes/ImportFile.tsx";
import { useExport } from "../routes/exporting.ts";
import { openDraft } from "../routes/model.ts";
import { routeDetailPath } from "../routes/address.ts";
import { Menu } from "../screens/Menu.tsx";
import { Refused } from "../screens/Refused.tsx";
import type { ScreenWrite } from "../screens/write.ts";
import { Badge, Button } from "../ui/kit.tsx";
import { ruleFoot } from "./dates.ts";
import { ancestorsOf, nodesByPath, type Graph, type Tree } from "./graph.ts";
import { NoticeList } from "./RouteNotices.tsx";
import type { Authored } from "./target.ts";
import { subjectNode, type Violation } from "./violations.ts";
import { usePublishPreview } from "./write.ts";

type Notice = Schema<"Notice">;

/** What publishing the draft would be refused for (A15), read as the engine would answer it on the draft as it stands. */
export function useDraftViolations(authored: Authored): Violation[] {
  const preview = usePublishPreview(authored);
  const [found, setFound] = useState<{ graph: Graph; violations: Violation[] } | undefined>(undefined);
  useEffect(() => {
    let live = true;
    void preview().then((previewed) => {
      if (live) {
        setFound({ graph: authored.graph, violations: previewed.outcome === "rejected" && previewed.rejection.rejection === "invalid" ? previewed.rejection.violations : [] });
      }
    });
    return () => {
      live = false;
    };
    // The draft's graph is what is read; the previewer is made afresh each render.
  }, [authored.graph]);
  return found?.graph === authored.graph ? found.violations : [];
}

/** "Draft (from v3)", or "Draft (first version)" for a route with nothing published. */
function draftWords(route: Route): string {
  const from = route.draft?.extends;
  return from == null ? "Draft (first version)" : `Draft (from v${String(from)})`;
}

export interface RouteHeaderProps {
  route: Route;
  /** The graph shown: the draft, or the published version's number. */
  of: "draft" | number;
  write: ScreenWrite;
  /** Why Publish is off, when it is. */
  blocked: string | undefined;
  /** Publishes the draft, keeping its notices to show (A20); none for a version. */
  onPublish: (() => void) | undefined;
  onImported: ImportedHandler;
  /** The nodes' titles, for the assistant's view of the draft. */
  titleOf: (key: string) => string | undefined;
}

/** One line: the route's name, which graph this is, Publish when it is a draft, and the `⋯`. */
export function RouteHeader({ route, of, write, blocked, onPublish, onImported, titleOf }: RouteHeaderProps) {
  const id = route.header.id;
  const exporting = useExport(id);
  const drafting = of === "draft";
  const send = (mutation: Schema<"Mutation">) => void write.run({ target: { route: id }, baseRevision: route.revision, mutations: [mutation] });
  return (
    <section className="stack route-header" aria-label={route.header.name} data-testid="route-header">
      <div className="row">
        {route.header.kind === "segment" ? <span className="label" data-testid="route-kind">Segment</span> : null}
        <h1 data-testid="route-name">{route.header.name}</h1>
        <Badge tone={drafting ? "warn" : "plain"} data-testid="route-graph" data-status={String(of)}>
          {drafting ? draftWords(route) : `Version ${String(of)}`}
        </Badge>
        <span className="spacer" />
        <AssistantDock target={{ route: id }} titleOf={titleOf} />
        {drafting ? (
          <Button primary disabled={write.disabled || blocked !== undefined} title={blocked} data-testid="publish-draft" onClick={onPublish}>
            Publish
          </Button>
        ) : route.draft == null ? (
          <Button primary disabled={write.disabled} data-testid="open-draft-button" onClick={() => { send(openDraft()); }}>
            Open a draft to edit
          </Button>
        ) : null}
        <Menu label="Route actions" testId="route-menu" align="end" trigger={<span aria-hidden="true">⋯</span>}>
          {(close) => (
            <>
              <Link role="menuitem" className="menu-item" to={routeDetailPath(id)} onClick={close}>
                Versions and journeys
              </Link>
              <button type="button" role="menuitem" className="menu-item" onClick={() => { close(); void exporting(drafting ? undefined : of); }}>
                {drafting ? "Export the draft" : "Export this version"}
              </button>
              <ImportFile label="Import into draft" onImported={(file, notices) => { close(); onImported(file, notices); }} />
              {drafting ? (
                <button type="button" role="menuitem" className="menu-item" disabled={write.disabled} data-testid="discard-draft" onClick={() => { close(); send({ op: "discard_draft" }); }}>
                  Discard draft
                </button>
              ) : null}
            </>
          )}
        </Menu>
      </div>
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
    </section>
  );
}

/** What an import or a publish left, under its own label and apart from any violation (A20). */
export function ResultBand({ words, notices, hrefOf, onDismiss }: { words: string; notices: readonly Notice[]; hrefOf: ((node: string) => string) | undefined; onDismiss: () => void }) {
  return (
    <div className="callout stack" role="status" data-testid="route-result">
      <span className="row">
        <strong>{words}</strong>
        <span className="spacer" />
        <Button ghost onClick={onDismiss}>Dismiss</Button>
      </span>
      <NoticeList notices={notices} hrefOf={hrefOf} />
    </div>
  );
}

export interface DraftCardProps {
  violations: readonly Violation[];
  notices: readonly Notice[];
  hrefOf: (node: string) => string;
  write: ScreenWrite;
  blocked: string | undefined;
  onPublish: () => void;
  onDiscard: () => void;
  titleOf: (key: string) => string;
}

/**
 * The inspector's empty state on a route draft: what blocks publishing (violations, A15), what
 * is only advisory (notices, A20, apart under their own label), Publish and Discard.
 */
export function DraftCard({ violations, notices, hrefOf, write, blocked, onPublish, onDiscard, titleOf }: DraftCardProps) {
  return (
    <aside className="detail-panel panel stack draft-card" aria-label="The draft" data-testid="draft-card">
      <h2>The draft</h2>
      {violations.length === 0 ? (
        <p className="muted" data-testid="draft-clear">Nothing blocks publishing.</p>
      ) : (
        <section className="stack" aria-label="Blocking publish" data-testid="draft-violations">
          <Badge tone="bad">Blocks publishing {violations.length}</Badge>
          <ul className="stack">
            {violations.map((violation, at) => {
              const node = subjectNode(violation);
              return (
                <li key={at} data-testid="violation" data-code={violation.code} data-node={node}>
                  {node === undefined ? null : <strong>{titleOf(node)}: </strong>}
                  {violation.message}
                  {node === undefined ? null : (
                    <>
                      {" "}
                      <Link to={hrefOf(node)}>Show</Link>
                    </>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      )}
      <NoticeList notices={notices} hrefOf={hrefOf} />
      <span className="row">
        <Button primary disabled={write.disabled || blocked !== undefined} title={blocked} onClick={onPublish}>
          Publish
        </Button>
        <Button disabled={write.disabled} onClick={onDiscard}>
          Discard draft
        </Button>
      </span>
    </aside>
  );
}

/** The reason Publish is off while the draft breaks a rule, or nothing. */
export function blockedWords(violations: readonly Violation[]): string | undefined {
  return violations.length === 0 ? undefined : `${String(violations.length)} ${violations.length === 1 ? "thing blocks" : "things block"} publishing: see the draft card.`;
}

/** The List projection of a route's graph: its nodes in tree order, each with its path and its date rule in words (C9's rows, with no state). */
export function RouteList({ tree, selected, unanchored, onOpen }: { tree: Tree; selected: string | undefined; unanchored: ReadonlySet<string>; onOpen: ((key: string) => void) | undefined }): ReactNode {
  const nodes = nodesByPath(tree);
  return (
    <ul className="route-list" data-testid="route-list">
      {nodes.map((node) => {
        const foot = ruleFoot(node, tree);
        return (
          <li key={node.key} className="route-list-row" data-testid="route-row" data-node={node.key} data-kind={node.kind} aria-current={node.key === selected ? "true" : undefined} style={{ paddingLeft: `calc(var(--space-4) * ${String(ancestorsOf(tree, node.key).length)})` }}>
            <span className="node-kind">{node.kind}</span>
            {onOpen === undefined ? (
              <span className="route-list-title">{node.title}</span>
            ) : (
              <button type="button" className="link route-list-title" data-testid="row-open" onClick={() => { onOpen(node.key); }}>
                {node.title}
              </button>
            )}
            {foot === undefined ? null : <span className="muted small">{foot}</span>}
            {unanchored.has(node.key) ? <span className="node-notice">No path to final</span> : null}
          </li>
        );
      })}
    </ul>
  );
}
