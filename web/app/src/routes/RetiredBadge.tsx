// The Retired chip of a route, the same on the index and on the route's own page.
import { Badge } from "../ui/kit.tsx";
import { Tip } from "../ui/Tip.tsx";

/** What the Retired chip says: what a retired route is hidden from. */
export function RetiredBadge({ kind }: { kind: "process" | "segment" | undefined }) {
  return (
    <Tip tip={kind === "segment" ? "Hidden from new insertions" : "Hidden from new journeys"} tap>
      <Badge data-testid="retired">Retired</Badge>
    </Tip>
  );
}
