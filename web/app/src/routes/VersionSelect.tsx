// C17: the version selector in a route's header: which graph the page's views show, the draft or
// a published version, as a menu of places (DESIGN, Popover and menu) in the place of a badge. Each
// item is a link with a real address: a version's is always written (`?version=<n>`, the latest
// included, because a version never changes and its link must keep meaning it); the draft's
// has no `version`, the route as it stands. Each version says when it was published and how
// many journeys are on it, which is the route's publishing history.
import { Link, useLocation } from "react-router";

import type { Route } from "../data/host.ts";
import { Menu } from "../screens/Menu.tsx";
import { dateWords } from "../timeline/model.ts";
import { routePath, spotOf } from "./address.ts";
import { useRouteRead } from "./RouteCard.tsx";
import { versionRows } from "./model.ts";

/** "Draft (from version 3)", or "Draft (first version)" for a route with nothing published. */
export function draftWords(route: Route): string {
  const from = route.draft?.extends;
  return from == null ? "Draft (first version)" : `Draft (from version ${String(from)})`;
}

/** `4 journeys`, `1 journey`, `no journeys`. */
function journeyWords(count: number): string {
  return count === 0 ? "no journeys" : `${String(count)} ${count === 1 ? "journey" : "journeys"}`;
}

export function VersionSelect({ route, of }: { route: Route; of: "draft" | number }) {
  const id = route.header.id;
  const { search } = useLocation();
  const read = useRouteRead(id);
  const rows = new Map((read === undefined ? [] : versionRows(read)).map((row) => [row.version, row] as const));
  const published = [...(route.versions ?? [])].sort((left, right) => right - left);
  const words = of === "draft" ? draftWords(route) : `Version ${String(of)}`;
  const today = new Date().toISOString().slice(0, 10);
  const to = (version: number | undefined) => routePath(id, { version, screen: spotOf(new URLSearchParams(search)).screen }, search);
  return (
    <Menu label={words} title="Which version the views show: the draft, or a published version" testId="version-select" className="no-print version-select" trigger={of === "draft" ? <>Draft<span className="version-from">{words.slice("Draft".length)}</span> ▾</> : <>{words} ▾</>}>
      {(close) => (
        <>
          {route.draft == null ? null : (
            <Link role="menuitem" className="menu-item" aria-current={of === "draft" ? "page" : undefined} data-testid="version-draft" to={to(undefined)} onClick={close}>
              {draftWords(route)}
            </Link>
          )}
          {published.map((version) => {
            const row = rows.get(version);
            return (
              <Link key={version} role="menuitem" className="menu-item" aria-current={of === version ? "page" : undefined} data-testid={`version-${String(version)}`} to={to(version)} onClick={close}>
                Version {version}
                {row === undefined ? null : (
                  <small className="mono">
                    {dateWords(row.publishedAt.slice(0, 10), today)}, {journeyWords(row.journeys.length)}
                  </small>
                )}
              </Link>
            );
          })}
        </>
      )}
    </Menu>
  );
}
