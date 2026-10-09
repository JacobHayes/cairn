// C8: a node's children as a checklist. Work (actions and deliverables) is checked off and
// unchecked in place, each one transition patch (complete, reopen); any other child shows its
// state and opens its own detail.
import { statusTone, statusWord } from "../status/words.ts";
import { Badge } from "../ui/kit.tsx";
import { foldKey } from "./folds.ts";
import { isTerminal, transition, type Child, type NodeDetail, type Ready } from "./model.ts";
import { NodeLink, Section } from "./parts.tsx";
import { Rejected } from "./Rejected.tsx";
import { ownerName } from "./sentence.ts";
import { dueLine } from "./words.ts";
import { useNodeWrite, type NodeWrite } from "./write.ts";

const work = (child: Child) => child.kind === "action" || child.kind === "deliverable";

function Item({ view, child, write }: { view: Ready; child: Child; write: NodeWrite }) {
  const done = child.state === "done";
  // The one fact on the right: the date while it is open, else who owns it.
  const due = isTerminal(child.state) ? undefined : dueLine(child.kind, view.derived.nodes[child.key]?.dates.due?.date, view.derived.today);
  const fact = due ?? ownerName(view, child.key);
  return (
    <li data-testid="child" data-node={child.key} data-status={child.state}>
      <span className="checklist-mark">
        {work(child) && child.state !== "skipped" ? (
          <input
            type="checkbox"
            aria-label={`${done ? "Reopen" : "Complete"} ${child.title}`}
            checked={done}
            disabled={write.disabled}
            onChange={() => void write.run([transition(child.key, done ? "reopen" : "complete")])}
          />
        ) : null}
      </span>
      <NodeLink view={view} node={child.key} /> <Badge tone={statusTone(child.displayState)}>{statusWord(child.displayState, child.kind)}</Badge>
      {fact === undefined ? null : <span className="muted small checklist-fact">{fact}</span>}
    </li>
  );
}

export function Checklist({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `checklist:${detail.node.key}`, detail.node.key);
  const { children } = detail;
  if (children.length === 0) {
    return null;
  }
  return (
    <Section title="Contents" summary={String(children.length)} open fold={foldKey(detail.node.kind, "contents")} testId="checklist">
      <ul className="checklist contents">
        {children.map((child) => (
          <Item key={child.key} view={view} child={child} write={write} />
        ))}
      </ul>
      <Rejected view={view} write={write} />
    </Section>
  );
}
