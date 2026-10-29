// The Stages list of a journey's card and of a route's: a row per stage, the whole row a target.
import { usePointRow } from "../canvas/pointed.tsx";
import { progressWords } from "../canvas/words.ts";
import { NodeLink, Section } from "../detail/parts.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import type { DisplayState } from "../status/words.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge, Meter } from "../ui/kit.tsx";

/** What a row says of its stage: a journey's stage has a state and progress, a route's has only what the route defines. */
export interface StageRowData {
  key: string;
  title: string;
  displayState?: DisplayState | undefined;
  done?: number | undefined;
  total?: number | undefined;
  /** The row's one fact at the right: a journey's due date, a route's days. */
  due: string | undefined;
  /** The class that says how near the due date is. */
  dueInk?: string | undefined;
}

/** One stage as a row, the whole row a target: choosing it selects the stage and frames it on the canvas; pointing at it outlines the stage there. */
function StageRow({ stage, lower }: { stage: StageRowData; lower: () => void }) {
  const pointing = usePointRow({ kind: "node", id: stage.key });
  const { displayState, done, total } = stage;
  const progressed = done !== undefined && total !== undefined;
  const due = stage.due === undefined ? null : <span className={["stage-row-due", "muted", stage.dueInk].filter(Boolean).join(" ")}>{stage.due}</span>;
  return (
    <li className="stage-row" data-testid="stage-row" data-node={stage.key} data-state={displayState} {...pointing}>
      <span className="stage-row-head">
        <NodeLink title={stage.title} node={stage.key} state={{ reveal: { key: stage.key, how: "frame" } }} onClick={lower} />
        {displayState === undefined ? null : <Badge tone={statusTone(displayState)}>{statusWord(displayState, "group")}</Badge>}
        {/* With no progress to show, the one fact shares the title's line. */}
        {progressed ? null : due}
      </span>
      {progressed ? (
        <span className="stage-row-foot muted">
          <Meter done={done} of={total} />
          <span>{progressWords(done, total)}</span>
          {due}
        </span>
      ) : null}
    </li>
  );
}

/** The folded Stages section; nothing for a graph without stages. `framing`: the view is the graph, where choosing a stage frames it. */
export function StagesSection({ stages, framing }: { stages: readonly StageRowData[]; framing: boolean }) {
  const actions = useFrameActions();
  // Where the stage is framed on a canvas under a sheet, it would be framed in no room at all: the sheet drops to its peek, where a card picked on the canvas opens it too.
  const lower = () => { if (framing) { actions?.snap("peek"); } };
  if (stages.length === 0) {
    return null;
  }
  return (
    <Section title="Stages" summary={String(stages.length)} testId="card-stages">
      <ul className="plain-list stage-rows">
        {stages.map((stage) => (
          <StageRow key={stage.key} stage={stage} lower={lower} />
        ))}
      </ul>
    </Section>
  );
}
