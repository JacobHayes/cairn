// B4 in a journey: a route-copied node's local-edit markers, each with "reset to route", which
// writes the route's value back and clears the marker in one patch; and the journey's
// tombstones (route nodes it removed), each restorable as a local copy of what was removed
// (decisions/2026-10-07-restoring-a-tombstone-adds-a-local-copy-under-fresh-keys.md). Both
// read the route version the journey follows, which never changes once published, so it is
// fetched once per tab.
import { useEffect, useState } from "react";

import type { RouteVersion } from "../data/host.ts";
import { useSession } from "../data/react.ts";
import { Badge, Button } from "../ui/kit.tsx";
import type { GraphNode } from "./graph.ts";
import { editWords, resetMutations, restoreMutations, tombstonesOf } from "./local-edits.ts";
import { Refusal } from "./StructureEditors.tsx";
import type { Authored, Lineage } from "./target.ts";
import { useAuthorWrite } from "./write.ts";

/** Published versions read in this tab: immutable (A11), so each is fetched once. */
const VERSIONS = new Map<string, Promise<RouteVersion>>();

/** The route version a journey follows, once read; none for an empty journey. */
export function useLineageVersion(lineage: Lineage | undefined): RouteVersion | undefined {
  const { host } = useSession();
  const [version, setVersion] = useState<RouteVersion | undefined>(undefined);
  const name = lineage === undefined ? undefined : `${lineage.route}@${String(lineage.version)}`;
  useEffect(() => {
    if (lineage === undefined || name === undefined) {
      return;
    }
    let live = true;
    let read = VERSIONS.get(name);
    if (read === undefined) {
      read = host.routeVersion(lineage.route, lineage.version);
      VERSIONS.set(name, read);
      read.catch(() => VERSIONS.delete(name));
    }
    read.then(
      (found) => {
        if (live) {
          setVersion(found);
        }
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
    // `name` names the lineage.
  }, [host, name]);
  return version?.route === lineage?.route && version?.version === lineage?.version ? version : undefined;
}

/** B4: the node's markers, each with its reset to the route. */
export function LocalEdits({ authored, node }: { authored: Authored; node: GraphNode }) {
  const write = useAuthorWrite(authored);
  const version = useLineageVersion(authored.lineage);
  const record = authored.graph.state?.nodes?.[node.key];
  const markers = authored.graph.state?.local_edits?.[node.key] ?? [];
  if (record?.provenance !== "from_route") {
    return <span className="muted" data-testid="local-edits" data-status={record?.provenance ?? "local"}>{record?.provenance === "orphaned" ? "Orphaned: its route version no longer has it." : "Local to this journey: nothing to reset."}</span>;
  }
  const route = (version?.graph.nodes ?? []).find((each) => each.key === node.key);
  return (
    <div className="stack" data-testid="local-edits" data-status="from_route">
      {markers.length === 0 ? <span className="muted">As the route has it.</span> : null}
      <ul className="detail-list">
        {markers.map((marker) => {
          const reset = resetMutations(node, route, marker);
          return (
            <li key={JSON.stringify(marker)} className="row" data-testid="local-edit" data-edit={JSON.stringify(marker)}>
              <Badge tone="warn">edited here</Badge>
              <span>{editWords(marker, authored.tree)}</span>
              {reset === undefined ? (
                <span className="muted">{version === undefined ? "reading the route..." : "settled at the next upgrade"}</span>
              ) : (
                <Button disabled={write.disabled} onClick={() => void write.run(reset)}>
                  Reset to route
                </Button>
              )}
            </li>
          );
        })}
      </ul>
      <Refusal write={write} />
    </div>
  );
}

/** B4: the route nodes the journey removed, each restorable as a local copy. */
export function Tombstones({ authored }: { authored: Authored }) {
  const write = useAuthorWrite(authored);
  const version = useLineageVersion(authored.lineage);
  if (authored.lineage === undefined || (authored.graph.state?.tombstones ?? []).length === 0) {
    return null;
  }
  const stones = version === undefined ? [] : tombstonesOf(authored.graph, version.graph);
  return (
    <details className="detail-section" data-testid="tombstones">
      <summary>
        <span className="detail-section-title">Removed from the route</span>
        <span className="muted"> {stones.length} kept from coming back at an upgrade</span>
      </summary>
      <ul className="detail-list detail-section-body">
        {stones.map((stone) => (
          <li key={stone.node.key} className="row" data-testid="tombstone" data-node={stone.node.key}>
            <span>
              {stone.node.title} <span className="muted mono">{stone.path}</span>
              {stone.beneath === 0 ? "" : `, with ${String(stone.beneath)} beneath it`}
            </span>
            {stone.occupied ? (
              <span className="muted" data-testid="restored">Something holds its place now.</span>
            ) : (
              <Button disabled={write.disabled || version === undefined} onClick={() => void write.run(restoreMutations(authored.graph, version?.graph ?? {}, stone.node.key))}>
                Restore as a local copy
              </Button>
            )}
          </li>
        ))}
      </ul>
      <Refusal write={write} />
    </details>
  );
}
