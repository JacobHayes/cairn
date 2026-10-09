// C8: the node detail panel every later screen opens, for one node of a derived journey. It
// reads only the journey's document and its local derive (ARCHITECTURE, Web UI: the browser
// has every explanation), so it follows the journey live (H6) with nothing of its own to
// fetch. Sections start folded where they explain rather than act (progressive disclosure).
import type { ReactNode } from "react";
import { Link, useLocation } from "react-router";

import { TitleEditor } from "../screens/TitleEditor.tsx";
import { DecisionAffects } from "../decisions/DecisionView.tsx";
import { Actions } from "./Actions.tsx";
import { AttachmentList } from "./Attachments.tsx";
import { Checklist } from "./Checklist.tsx";
import { DatesSection } from "./DatesSection.tsx";
import { ForceInclude, ParticipationEditor, PinEditor, SnoozeEditor, WeightEditor } from "./editors.tsx";
import { NodeHistory } from "./History.tsx";
import { nodeDetail, type NodeDetail, type Ready } from "./model.ts";
import { FoldedSections, screenPath } from "./parts.tsx";
import { Rejected } from "./Rejected.tsx";
import { ResourceList } from "./Resources.tsx";
import { About, Blocking, Header, Participations, Priority, Relevance } from "./sections.tsx";
import { useNodeWrite } from "./write.ts";

/** F7, E3: the dates with their pin editor; F6: a shortfall's move sent as one patch. */
function Dates({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `dates:${detail.node.key}`, detail.node.key);
  return (
    <DatesSection
      view={view}
      detail={detail}
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

/**
 * `extra` sits under the header: the node's structure, in a journey's edit mode (5.6), or on a
 * card its way past breaking down. `folded` starts every section closed, as a card shows them.
 */
export function NodeDetailPanel({ view, nodeKey, extra, folded = false }: { view: Ready; nodeKey: string; extra?: ReactNode; folded?: boolean }) {
  const journey = view.journey.header.id;
  const detail = nodeDetail(view, nodeKey);
  // Closing keeps the screen it is open on and what that screen shows (5.2).
  const { pathname, search } = useLocation();
  const close = { pathname: screenPath(pathname), search };
  if (detail === undefined) {
    return (
      <aside className="detail-panel panel" data-testid="node-detail-missing">
        <p className="callout">This journey has no node {nodeKey}.</p>
        <Link to={close}>Close</Link>
      </aside>
    );
  }
  return (
    <FoldedSections value={folded}>
      <aside className="detail-panel panel stack" aria-label={detail.node.title} data-testid="node-detail" data-node={nodeKey}>
        <Header view={view} detail={detail} />
        {extra}
        <div data-testid="rename">
          <TitleEditor journey={journey} node={nodeKey} title={detail.node.title} revision={view.journey.revision} showTitle={false} />
        </div>
        <Actions view={view} detail={detail} />
        {detail.node.kind === "decision" ? <DecisionAffects ready={view} node={nodeKey} /> : null}
        <About view={view} detail={detail} />
        <Checklist view={view} detail={detail} />
        <AttachmentList view={view} detail={detail} />
        <ResourceList view={view} detail={detail} />
        <Dates view={view} detail={detail} />
        <Blocking view={view} detail={detail} edit={<SnoozeEditor view={view} detail={detail} />} />
        <Relevance view={view} detail={detail} edit={<ForceInclude view={view} detail={detail} />} />
        <Priority view={view} detail={detail} edit={<WeightEditor view={view} detail={detail} />} />
        <Participations view={view} detail={detail} edit={<ParticipationEditor view={view} detail={detail} />} />
        <NodeHistory view={view} detail={detail} />
      </aside>
    </FoldedSections>
  );
}
