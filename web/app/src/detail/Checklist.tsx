// C8: a node's children as a checklist. Work (actions and deliverables) is checked off and
// unchecked in place, each one transition patch (complete, reopen); any other child shows its
// state and opens its own detail.
import { statusTone, statusWord } from "../status/words.ts";
import { Badge } from "../ui/kit.tsx";
import { isTerminal, transition, type Child, type NodeDetail, type Ready } from "./model.ts";
import { NodeLink, Section } from "./parts.tsx";
import { Rejected } from "./Rejected.tsx";
import { useNodeWrite, type NodeWrite } from "./write.ts";

const work = (child: Child) => child.kind === "action" || child.kind === "deliverable";

function Item({ view, child, write }: { view: Ready; child: Child; write: NodeWrite }) {
  const done = child.state === "done";
  return (
    <li data-testid="child" data-node={child.key} data-status={child.state}>
      {work(child) && child.state !== "skipped" ? (
        <input
          type="checkbox"
          aria-label={`${done ? "Reopen" : "Complete"} ${child.title}`}
          checked={done}
          disabled={write.disabled}
          onChange={() => void write.run([transition(child.key, done ? "reopen" : "complete")])}
        />
      ) : null}{" "}
      <NodeLink view={view} node={child.key} /> <Badge tone={statusTone(child.displayState)}>{statusWord(child.displayState, child.kind)}</Badge>
    </li>
  );
}

export function Checklist({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `checklist:${detail.node.key}`, detail.node.key);
  const { children } = detail;
  if (children.length === 0) {
    return null;
  }
  const finished = children.filter((child) => isTerminal(child.state)).length;
  return (
    <Section title="Contents" summary={`${String(finished)} of ${String(children.length)} finished`} open testId="checklist">
      <ul className="checklist">
        {children.map((child) => (
          <Item key={child.key} view={view} child={child} write={write} />
        ))}
      </ul>
      <Rejected view={view} write={write} />
    </Section>
  );
}
