// C8: a node's children, its Contents. Work (actions and deliverables) is checked off and
// unchecked in place, each one transition patch (complete, reopen); any other child shows its
// state and opens its own detail.
import { useCaller } from "../data/react.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge } from "../ui/kit.tsx";
import { foldKey } from "./folds.ts";
import { accepts, type Child, dueWordsOf, isWork, type NodeDetail, type Ready, transition } from "./model.ts";
import { NodeLink, Section } from "./parts.tsx";
import { Rejected } from "./Rejected.tsx";
import { ownerName } from "./sentence.ts";
import { useNodeWrite, type NodeWrite } from "./write.ts";
import { Words } from "../ui/Words.tsx";

export function ContentsRow({ view, child, write, viewer }: { view: Ready; child: Child; write: NodeWrite; viewer: readonly string[] | undefined }) {
  const done = child.state === "done";
  // The one fact on the right: the date while it is open, else who owns it.
  const due = dueWordsOf(view, child);
  const fact = due ?? ownerName(view, child.key, viewer);
  return (
    <li data-testid="child" data-node={child.key} data-state={child.displayState}>
      {/* The mark's cell is its label, so a finger has the cell's whole height and width to tap. */}
      <label className="contents-mark">
        {write.writable && isWork(child) && accepts(view.derived.nodes[child.key], done ? "reopen" : "complete") ? (
          <input
            type="checkbox"
            aria-label={`${done ? "Reopen" : "Complete"} ${child.title}`}
            checked={done}
            disabled={write.disabled}
            onChange={() => void write.run([transition(child.key, done ? "reopen" : "complete")])}
          />
        ) : null}
      </label>
      <NodeLink view={view} node={child.key} /> <Badge tone={statusTone(child.displayState)}>{statusWord(child.displayState, child.kind)}</Badge>
      {fact === undefined ? null : <span className="muted small contents-fact">{due === undefined ? fact : <Words text={due} />}</span>}
    </li>
  );
}

export function Contents({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `contents:${detail.node.key}`, detail.node.key);
  const { caller } = useCaller();
  const { children } = detail;
  if (children.length === 0) {
    return null;
  }
  return (
    <Section title="Contents" summary={String(children.length)} open fold={foldKey(detail.node.kind, "contents")} testId="contents">
      <ul className="plain-list contents">
        {children.map((child) => (
          <ContentsRow key={child.key} view={view} child={child} write={write} viewer={caller?.identities} />
        ))}
      </ul>
      <Rejected view={view} write={write} />
    </Section>
  );
}
