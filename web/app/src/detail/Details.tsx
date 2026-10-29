// C8, J4: the inspector's last section, folded: the facts about a node that nothing is done
// with, in one place. Where it came from (a segment) and what
// happened to it, in sentences naming who did each thing. Open it and the history is read;
// folded, nothing is fetched.
import { Origin } from "./Facts.tsx";
import { foldKey, useFold } from "./folds.ts";
import { NodeHistory } from "./History.tsx";
import type { NodeDetail, Ready } from "./model.ts";
import { Section } from "./parts.tsx";

export function Details({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const fold = foldKey(detail.node.kind, "details");
  const open = useFold(fold, false);
  return (
    <Section title="Details" fold={fold} testId="details">
      <Origin view={view} detail={detail} />
      <NodeHistory view={view} detail={detail} open={open} />
    </Section>
  );
}
