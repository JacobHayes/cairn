// C1: a route node's date relationships in both directions (its due-by and not-before rules, its
// stage's close, an opening that holds dates only, a decision that sets a milestone's date), each as
// the sentence the canvas says on hover, with the other node as a link: where the date lines the
// canvas draws for the selected node are read and followed by keyboard or touch. The level that
// selects the node carries them, read with every kind shown and nothing folded, so each end is
// the node itself.
import { dateLinkRowWords } from "../canvas/dates.ts";
import { useProjected, type Held } from "../canvas/hooks.ts";
import { valueOf } from "../data/keyed-read.ts";
import { KINDS } from "../detail/model.ts";
import { LinkedDateRow, NodeLink, Section } from "../detail/parts.tsx";
import { titleIn, type Tree } from "./graph.ts";

export function LinkedDates({ held, tree, nodeKey }: { held: Held | undefined; tree: Tree; nodeKey: string }) {
  const level = valueOf(useProjected(held, { projection: "level", shown: KINDS, selected: nodeKey }));
  const seen = new Set<string>();
  const links = (level?.date_links ?? []).flatMap((found) => found.links).filter((link) => {
    const id = JSON.stringify(link);
    const direct = link.node === nodeKey || link.source === nodeKey;
    if (!direct || seen.has(id)) {
      return false;
    }
    seen.add(id);
    return true;
  });
  if (links.length === 0) {
    return null;
  }
  const words = (link: (typeof links)[number], other: string) => dateLinkRowWords(link, other, titleIn(tree, link.node), titleIn(tree, link.source), { decision: tree.byKey.get(link.node)?.kind === "decision" });
  return (
    <Section title="Linked dates" summary={String(links.length)} testId="linked-dates">
      <ul className="stack linked-dates">
        {links.map((link) => {
          const other = link.node === nodeKey ? link.source : link.node;
          return <LinkedDateRow key={id(link)} link={other} node={<NodeLink node={other} title={titleIn(tree, other)} />} words={words(link, other)} />;
        })}
      </ul>
    </Section>
  );
}

const id = (link: object) => JSON.stringify(link);
