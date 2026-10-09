// A20: a route draft's notices while it is authored: work with no chain to or from the final
// milestone, listed by the page's engine from the draft as it stands, so an edge drawn
// elsewhere on the canvas clears its notice as soon as the draft is read again. Advisory:
// nothing here blocks an edit or publishing. Each names its node with the path to open it by.
import type { Schema } from "@cairn/client";
import { useEffect, useState } from "react";
import { Link } from "react-router";

import { useSession } from "../data/react.ts";
import { Badge } from "../ui/kit.tsx";

type Found = { status: "ready"; notices: Schema<"Notice">[] } | { status: "failed"; message: string };

export function RouteNotices({ graph, hrefOf }: { graph: Schema<"Graph">; hrefOf: (node: string) => string }) {
  const { deriver } = useSession();
  const [found, setFound] = useState<Found | undefined>(undefined);
  useEffect(() => {
    let live = true;
    deriver.routeNotices({ graph }).then(
      (notices) => {
        if (live) {
          setFound({ status: "ready", notices });
        }
      },
      (thrown: unknown) => {
        if (live) {
          setFound({ status: "failed", message: thrown instanceof Error ? thrown.message : String(thrown) });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [deriver, graph]);
  if (found === undefined || (found.status === "ready" && found.notices.length === 0)) {
    return null;
  }
  if (found.status === "failed") {
    return <p className="muted">The draft's notices could not be read: {found.message}</p>;
  }
  return (
    <section className="stack callout" aria-label="Notices" data-testid="route-notices">
      <span className="row">
        <Badge tone="warn">Notices {found.notices.length}</Badge>
        <span className="muted">Advisory: publishing is not blocked.</span>
      </span>
      <ul className="stack">
        {found.notices.map((notice) => (
          <li key={`${notice.code}:${notice.node}`} data-testid="route-notice" data-node={notice.node} data-code={notice.code}>
            {notice.message}{" "}
            <Link to={hrefOf(notice.node)} className="mono">
              {notice.path}
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}
