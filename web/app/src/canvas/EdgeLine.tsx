// `EdgeLine`: one edge on the canvas (C1): an arrow from the requirement to the dependent,
// solid when an explicit `requires` stands behind it and dotted when every edge it stands for
// is implicit, with a small label naming an implicit gate's source and the full words on hover.
import { BaseEdge, EdgeLabelRenderer, getBezierPath, type Edge, type EdgeProps } from "@xyflow/react";

import { lineLook } from "./look.ts";
import type { Line } from "./model.ts";
import type { CanvasOverlay } from "./overlay.ts";

export type LineData = { line: Line; overlay: CanvasOverlay | undefined };
export type LineEdge = Edge<LineData, "line">;

const SOURCE_NAMES = { condition: "condition", stage_opening: "stage opening" } as const;

/** The short name of where an implicit line comes from: a condition, a stage opening, or both. */
function sourceLabel(line: Line): string | undefined {
  if (!line.implicit) {
    return undefined;
  }
  return [...new Set(line.sources.map((source) => SOURCE_NAMES[source.origin]))].join(", ");
}

export function EdgeLine({ id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, markerEnd, data }: EdgeProps<LineEdge>) {
  const [path, labelX, labelY] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });
  if (data === undefined) {
    return <BaseEdge id={id} path={path} />;
  }
  const { line, overlay } = data;
  const look = lineLook(line, overlay);
  const label = sourceLabel(line);
  const words = line.sources.map((source) => `${SOURCE_NAMES[source.origin]}: ${source.words}`).join("; ");
  return (
    <g data-testid="edge-line" data-from={line.from} data-to={line.to} data-implicit={String(line.implicit)} data-dash={look.dash ?? "solid"} className={look.classes.join(" ")}>
      <title>{words === "" ? "requires" : words}</title>
      <BaseEdge id={id} path={path} {...(markerEnd === undefined ? {} : { markerEnd })} style={look.dash === undefined ? {} : { strokeDasharray: look.dash }} />
      {label === undefined ? null : (
        <EdgeLabelRenderer>
          <div
            className={`edge-source ${look.classes.includes("line-dim") ? "line-dim" : ""}`}
            style={{ transform: `translate(-50%, -50%) translate(${String(labelX)}px, ${String(labelY)}px)` }}
            title={words}
            data-testid="edge-source"
            data-edge={line.id}
          >
            {label}
          </div>
        </EdgeLabelRenderer>
      )}
    </g>
  );
}
