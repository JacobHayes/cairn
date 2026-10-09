// A20: a route draft's notices while it is authored: work with no chain to or from the final
// milestone, listed by the page's engine from the draft as it stands, so an edge drawn
// elsewhere on the canvas clears its notice as soon as the draft is read again. Advisory:
// nothing here blocks an edit or publishing. They show on the card of each node (a hollow
// chip), in the draft card, and in an import or publish result, always apart from violations.
import type { Schema } from "@cairn/client";
import { useEffect, useState } from "react";
import { Link } from "react-router";

import { useSession } from "../data/react.ts";
import { Badge } from "../ui/kit.tsx";

type Notice = Schema<"Notice">;

interface Found {
  notices: Notice[];
  failed: string | undefined;
}

/** The draft's notices as the engine lists them now; none while they are being read, or when they cannot be. */
export function useRouteNotices(graph: Schema<"Graph"> | undefined): Found {
  const { deriver } = useSession();
  const [found, setFound] = useState<(Found & { graph: Schema<"Graph"> }) | undefined>(undefined);
  useEffect(() => {
    if (graph === undefined) {
      return undefined;
    }
    let live = true;
    deriver.routeNotices({ graph }).then(
      (notices) => {
        if (live) {
          setFound({ graph, notices, failed: undefined });
        }
      },
      (thrown: unknown) => {
        if (live) {
          setFound({ graph, notices: [], failed: thrown instanceof Error ? thrown.message : String(thrown) });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [deriver, graph]);
  return found !== undefined && found.graph === graph ? found : { notices: [], failed: undefined };
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
