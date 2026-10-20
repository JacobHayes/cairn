// The Retired chip of a route, the same on the index and on the route's own page.
import { Badge } from "../ui/kit.tsx";

/** What the Retired chip says: what a retired route is hidden from. */
export function RetiredBadge({ kind }: { kind: "process" | "segment" | undefined }) {
  return (
    <Badge data-testid="retired" title={kind === "segment" ? "Retired: hidden from new insertions" : "Retired: hidden from new journeys"}>
      Retired
    </Badge>
  );
}
