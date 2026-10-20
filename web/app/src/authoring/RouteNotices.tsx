// A20: a route draft's notices while it is authored: work with no chain to or from the final
// milestone, listed by the page's engine from the draft as it stands, so an edge drawn
// elsewhere on the canvas clears its notice as soon as the draft is read again. Advisory:
// nothing here blocks an edit or publishing. They show on the card of each node (a hollow
// chip), in the draft card, and in an import or publish result, always apart from violations.
import type { Schema } from "@cairn/client";
import { Link } from "react-router";

import type { Deriver } from "../data/host.ts";
import { PROJECTION, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import { Badge } from "../ui/kit.tsx";

type Notice = Schema<"Notice">;

interface Found {
  notices: Notice[];
  failed: string | undefined;
}

const NOTICES: ReadSource<{ deriver: Deriver; graph: Schema<"Graph"> }, Notice[]> = { policy: PROJECTION, read: ({ deriver, graph }) => deriver.routeNotices({ graph }) };

/** The draft's notices as the engine lists them now; none while they are being read, or when they cannot be. */
export function useRouteNotices(graph: Schema<"Graph"> | undefined): Found {
  const { deriver } = useSession();
  const { view } = useKeyedRead(NOTICES, graph === undefined ? undefined : { of: ["route-notices"], freshness: [graph], args: { deriver, graph } });
  if (view.status === "current") {
    return { notices: view.value, failed: undefined };
  }
  return { notices: [], failed: view.status === "failed" || view.status === "behind" ? view.error.message : undefined };
}

/** The notices under their own steel label, each with a way to open its node; nothing when there are none. */
export function NoticeList({ notices, hrefOf, heading }: { notices: readonly Notice[]; hrefOf: ((node: string) => string) | undefined; heading?: string }) {
  if (notices.length === 0) {
    return null;
  }
  return (
    <section className="stack" aria-label="Notices" data-testid="route-notices">
      <span className="row">
        <Badge tone="pending">{heading ?? "Notices"} {notices.length}</Badge>
        <span className="muted small">Advisory: publishing is not blocked.</span>
      </span>
      <ul className="stack">
        {notices.map((notice) => (
          <li key={`${notice.code}:${notice.node}`} data-testid="route-notice" data-node={notice.node} data-code={notice.code}>
            {notice.message}
            {hrefOf === undefined ? null : (
              <>
                {" "}
                <Link to={hrefOf(notice.node)}>Show</Link>
              </>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}
