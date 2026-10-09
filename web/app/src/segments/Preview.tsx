// C19: the incoming root and its wiring drawn on the graph at their proposed place, with proposal
// review's own canvas and marks (the Add tag, success hairline, the segment and version on the
// root's foot), so what the stepper shows is what review will.
import { useMemo } from "react";

import { useDeployment } from "../data/react.ts";
import { ProposalCanvas } from "../proposals/ProposalCanvas.tsx";
import { diffMarks, graphDiff, unionGraph } from "../proposals/model.ts";
import { noteOrigins } from "./model.ts";
import type { InsertPreview } from "./Stepper.tsx";

const NOBODY = (): void => undefined;

export function InsertionCanvas({ preview, domain, today }: { preview: InsertPreview; domain: string; today: string }) {
  const deployment = useDeployment();
  const diff = useMemo(() => graphDiff(preview.before, preview.after), [preview]);
  const marks = useMemo(() => noteOrigins(diffMarks(diff, []), preview.origins.members), [diff, preview]);
  const graphs = useMemo(() => [unionGraph(preview.before, preview.after), preview.after], [preview]);
  if (deployment === undefined) {
    return <p className="muted small">Laying out the canvas...</p>;
  }
  return <ProposalCanvas domain={`insert:${domain}`} graphs={graphs} marks={marks} edges={diff} origins={preview.origins.roots} dim={false} deployment={deployment} today={today} selected={undefined} onPick={NOBODY} />;
}
