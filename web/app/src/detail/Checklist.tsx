// G5: a node's own checklist, ticked and unticked in place, each tick one patch. A box shows
// what the held write sends while it is out (it outlives this panel), else what the journey
// records; the counts catch up when the write lands.
import { foldKey } from "./folds.ts";
import { checklistOf, type NodeDetail, type Ready } from "./model.ts";
import { Section } from "./parts.tsx";
import { Rejected } from "./Rejected.tsx";
import { useNodeWrite } from "./write.ts";

function ChecklistRow({ view, node, item, ticked: recorded, editable }: { view: Ready; node: string; item: { id: string; title: string }; ticked: boolean; editable: boolean }) {
  const write = useNodeWrite(view, `tick:${node}:${item.id}`, node);
  const sent = write.sending?.find((mutation) => mutation.op === "set_ticked" && mutation.item === item.id);
  const ticked = sent?.op === "set_ticked" ? sent.ticked : recorded;
  const attrs = { "data-testid": "checklist-item", "data-item": item.id, "data-ticked": ticked || undefined };
  return (
    <li>
      {editable && write.writable ? (
        <label className="checklist-item" {...attrs}>
          <input
            type="checkbox"
            checked={ticked}
            disabled={write.disabled}
            onChange={() => void write.run([{ op: "set_ticked", node, item: item.id, ticked: !ticked }])}
          />
          <span className="checklist-title">{item.title}</span>
        </label>
      ) : (
        <div className="checklist-item checklist-read" {...attrs}>
          <span aria-hidden="true">{ticked ? "✓" : ""}</span>
          <span className="checklist-title">{item.title}</span>
          <span className="checklist-state">{ticked ? "ticked" : "not ticked"}</span>
        </div>
      )}
      <Rejected view={view} write={write} />
    </li>
  );
}

export function Checklist({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const { key, kind } = detail.node;
  const items = checklistOf(view, key);
  if (items.length === 0) {
    return null;
  }
  const state = detail.derived.display_state;
  const editable = state !== "skipped" && state !== "not_relevant";
  const ticked = items.filter((item) => item.ticked).length;
  return (
    <Section title="Checklist" summary={`${String(ticked)} of ${String(items.length)} ticked`} open fold={foldKey(kind, "checklist")} testId="checklist">
      <ul className="checklist">
        {items.map((item) => (
          <ChecklistRow key={item.id} view={view} node={key} item={item} ticked={item.ticked} editable={editable} />
        ))}
      </ul>
    </Section>
  );
}
