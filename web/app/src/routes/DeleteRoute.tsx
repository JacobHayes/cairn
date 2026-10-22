// A19: hard deletion of a route or segment nothing uses: no journey follows any of its versions
// and no journey, draft or published version holds an insertion of it (the host's `in_use`, asked
// again when the deletion commits). One patch to the deployment naming the route at the revision
// shown, asked once from the route's `⋯` with the page's other writes set aside; once it is gone,
// the routes index is shown.
import { createContext } from "react";
import { useNavigate } from "react-router";

import type { Route } from "../data/host.ts";
import { useDeployment } from "../data/react.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button } from "../ui/kit.tsx";
import { ROUTES_INDEX_PATH } from "./address.ts";

/** Whether the route's page is asking to delete it: the header asks, and the page's other write actions (Publish, Discard, Open a draft) step aside until it is answered. */
export const DeletingRoute = createContext<{ asking: boolean; ask: (asking: boolean) => void }>({ asking: false, ask: () => undefined });

/** What the route's `⋯` calls deleting it. */
export function deleteRouteLabel(route: Route): string {
  return route.header.kind === "segment" ? "Delete the segment" : "Delete the route";
}

export function DeleteRoute({ route, onCancel }: { route: Route; onCancel: () => void }) {
  const deployment = useDeployment();
  const navigate = useNavigate();
  const { id, name, kind } = route.header;
  const write = useScreenWrite(`delete-route:${id}`, name);
  const remove = async () => {
    if (deployment === undefined) {
      return;
    }
    if (await write.run({ target: "deployment", baseRevision: deployment.revision, mutations: [{ op: "delete_route", route: id, revision: route.revision }] })) {
      void navigate(ROUTES_INDEX_PATH);
    }
  };
  const what = kind === "segment" ? "segment" : "route";
  return (
    <div className="stack callout" role="alertdialog" aria-label={`Delete ${name} for good?`} data-testid="delete-route">
      <strong>{`Delete ${name} for good?`}</strong>
      <span>{`Nothing uses this ${what}. Deleting removes it with its draft, every version and all of its history; only a record of who deleted it and when is kept. This cannot be undone.`}</span>
      <span className="row">
        <Button primary disabled={write.disabled || deployment === undefined} data-testid="delete-route-confirmed" onClick={() => void remove()}>
          {deleteRouteLabel(route)}
        </Button>
        <Button onClick={onCancel}>Cancel</Button>
      </span>
      {write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />}
    </div>
  );
}
