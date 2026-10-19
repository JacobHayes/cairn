// C8: the inspector's node detail (design 6): composed in the tab from the journey's document
// and its local derive (ARCHITECTURE, Web UI: the browser has every explanation), so it follows
// the journey live (H6) with nothing of its own to fetch but the history. It leads with the node
// (kind and place, display state, title, owner, date), one plain sentence saying what its state
// means, and the one thing to do about it, a decision's form being its answer; everything else
// is a folded row showing its name and a count (DESIGN, Approachable by default).
import type { ReactNode } from "react";
import { Link, useLocation } from "react-router";

import { graphPath, graphViewFrom } from "../canvas/settings.ts";
import { useNodeAddresses } from "../journeys/address.ts";
import { Markdown } from "../ui/markdown.tsx";
import { Actions } from "./Actions.tsx";
import { Affects } from "./Affects.tsx";
import { AttachmentList } from "./Attachments.tsx";
import { Checklist } from "./Checklist.tsx";
import { Connections } from "./Connections.tsx";
import { DatesSection } from "./DatesSection.tsx";
import { PinEditor } from "./editors.tsx";
import { Details } from "./Details.tsx";
import { People } from "./Facts.tsx";
import { foldKey } from "./folds.ts";
import { Header } from "./Header.tsx";
import { nodeDetail, type NodeDetail, type Ready } from "./model.ts";
import { FoldedSections } from "./parts.tsx";
import { Pieces } from "./Pieces.tsx";
import { useRankRow } from "./reads.ts";
import { useRename } from "./rename.ts";
import { Rejected } from "./Rejected.tsx";
import { ResourceList } from "./Resources.tsx";
import { positionOf, sentenceOf, type RankFacts } from "./sentence.ts";
import { WhyRank } from "./WhyRank.tsx";
import { useNodeWrite } from "./write.ts";

/** F7, E3: the dates with their pin editor; F6: a shortfall's move sent as one patch. Open on a shortfall or when overdue. */
function Dates({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `dates:${detail.node.key}`, detail.node.key);
  return (
    <DatesSection
      view={view}
      detail={detail}
      open={detail.derived.dates.shortfall != null || detail.derived.overdue === true}
      fold={foldKey(detail.node.kind, "dates")}
      onMove={(move) => void write.run([move])}
      pinEditor={
        <>
          <Rejected view={view} write={write} />
          <PinEditor view={view} detail={detail} />
        </>
      }
    />
  );
}

/** The one plain sentence: what the node's state means, with links. */
function Sentence({ view, detail, rank }: { view: Ready; detail: NodeDetail; rank: RankFacts }) {
  return (
    <p className="sentence" data-testid="detail-sentence">
      <Pieces view={view} pieces={sentenceOf(view, detail, rank)} />
    </p>
  );
}

/**
 * `extra` sits under the actions: the node's structure, in a journey's edit mode (5.6), or on a
 * card its way past breaking down. `folded` starts every section closed, as a card shows them.
 */
export function NodeDetailPanel({ view, nodeKey, extra, folded = false }: { view: Ready; nodeKey: string; extra?: ReactNode; folded?: boolean }) {
  const journey = view.journey.header.id;
  const detail = nodeDetail(view, nodeKey);
  const rename = useRename(view, nodeKey, detail?.node.title ?? nodeKey);
  const row = useRankRow(view, nodeKey);
  // Closing keeps the screen it is open on and what that screen shows (5.2).
  const { search } = useLocation();
  const addresses = useNodeAddresses();
  const close = addresses.close();
  if (detail === undefined) {
    return (
      <aside className="detail-panel panel" data-testid="node-detail-missing">
        <p className="callout">This item is not in the journey any more.</p>
        <Link to={close}>Close</Link>
      </aside>
    );
  }
  const { node } = detail;
  const position = positionOf(view, nodeKey);
  const description = node.description ?? "";
  return (
    <FoldedSections value={folded}>
      <aside className="detail-panel panel stack" aria-label={node.title} data-testid="node-detail" data-node={nodeKey}>
        <Header view={view} detail={detail} position={position} rename={rename} removeTo={graphPath(journey, { ...graphViewFrom(new URLSearchParams(search)), edit: true }, { node: nodeKey }, search)} />
        <div className="detail-lead">
          <Sentence view={view} detail={detail} rank={{ position, row }} />
          {node.kind === "decision" || description === "" ? null : <Markdown text={description} data-testid="description" />}
          <Actions view={view} detail={detail} onRename={rename.start} />
        </div>
        {extra}
        <div className="detail-folds">
          <Checklist view={view} detail={detail} />
          {node.kind === "decision" ? <Affects view={view} detail={detail} /> : null}
          <Connections view={view} detail={detail} />
          <ResourceList view={view} detail={detail} />
          <AttachmentList view={view} detail={detail} />
          <Dates view={view} detail={detail} />
          <WhyRank view={view} detail={detail} position={position} row={row} />
          <People view={view} detail={detail} />
          <Details view={view} detail={detail} />
        </div>
      </aside>
    </FoldedSections>
  );
}
