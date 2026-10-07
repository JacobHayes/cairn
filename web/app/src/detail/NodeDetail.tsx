// C8: the node detail panel every later screen opens, for one node of a derived journey. It
// reads only the journey's document and its local derive (ARCHITECTURE, Web UI: the browser
// has every explanation), so it follows the journey live (H6) with nothing of its own to
// fetch. Sections start folded where they explain rather than act (progressive disclosure).
import { useEffect, useRef } from "react";
import { Link, useLocation } from "react-router";

import { TitleEditor } from "../screens/TitleEditor.tsx";
import { Actions } from "./Actions.tsx";
import { AttachmentList } from "./Attachments.tsx";
import { Checklist } from "./Checklist.tsx";
import { DatesSection } from "./DatesSection.tsx";
import { ForceInclude, ParticipationEditor, PinEditor, SnoozeEditor, WeightEditor } from "./editors.tsx";
import { NodeHistory } from "./History.tsx";
import { nodeDetail, type NodeDetail, type Ready } from "./model.ts";
import { Rejected } from "./Rejected.tsx";
import { ResourceList } from "./Resources.tsx";
import { About, Blocking, Header, Participations, Priority, Relevance } from "./sections.tsx";
import { useNodeWrite } from "./write.ts";

/** F7, E3: the dates with their pin editor; F6: a shortfall's move sent as one patch. */
function Dates({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `dates:${detail.node.key}`);
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

/** The width below which the panel sits above the canvas (canvas/canvas.css, `.canvas-split`). */
const NARROW = "(max-width: 52rem)";

export function NodeDetailPanel({ view, nodeKey }: { view: Ready; nodeKey: string }) {
  const journey = view.journey.header.id;
  const detail = nodeDetail(view, nodeKey);
  const panel = useRef<HTMLElement>(null);
  // Closing keeps what the canvas shows (5.2).
  const close = { pathname: `/journeys/${journey}`, search: useLocation().search };
  // On a narrow screen the panel opens above the canvas, so bring it into view.
  useEffect(() => {
    if (globalThis.matchMedia(NARROW).matches) {
      panel.current?.scrollIntoView({ block: "start" });
    }
  }, [nodeKey]);
  if (detail === undefined) {
    return (
      <aside className="detail-panel panel" data-testid="node-detail-missing">
        <p className="callout">This journey has no node {nodeKey}.</p>
        <Link to={close}>Close</Link>
      </aside>
    );
  }
  return (
    <aside ref={panel} className="detail-panel panel stack" aria-label={detail.node.title} data-testid="node-detail" data-node={nodeKey}>
      <div className="row">
        <span className="shell-spacer" />
        <Link to={close} aria-label="Close the node detail">
          Close
        </Link>
      </div>
      <Header view={view} detail={detail} />
      <div data-testid="rename">
        <TitleEditor journey={journey} node={nodeKey} title={detail.node.title} revision={view.journey.revision} showTitle={false} />
      </div>
      <Actions view={view} detail={detail} />
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
  );
}
