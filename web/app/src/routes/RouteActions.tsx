// A11, A13, A19: what an author does to a route from its detail, each one route patch drafted
// against the revision shown: open a draft extending the latest version, publish it as the
// next version, discard it, retire the route or bring it back; and the draft exported, or a
// file imported as a new draft. A refusal shows inline.
import type { Schema } from "@cairn/client";
import { Link } from "react-router";

import type { Mutation } from "../data/writes.ts";
import { DEFAULT_VIEW } from "../canvas/settings.ts";
import { Refused, violates } from "../screens/Refused.tsx";
import { routeCanvasPath } from "../screens/RouteCanvasPage.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Badge, Button } from "../ui/kit.tsx";
import { ImportFile } from "./ImportFile.tsx";
import { openDraft, retireMutation, routeMoves } from "./model.ts";
import { useExport } from "./exporting.ts";

export function RouteActions({ route }: { route: Schema<"Route"> }) {
  const id = route.header.id;
  const write = useScreenWrite();
  const exportDraft = useExport(id);
  const moves = routeMoves(route);
  const send = (mutation: Mutation) => void write.run({ target: { route: id }, baseRevision: route.revision, mutations: [mutation] });
  const draft = route.draft ?? undefined;
  return (
    <section className="stack" aria-label="The draft" data-testid="route-actions">
      <span className="row" data-testid="draft" data-status={draft === undefined ? "none" : "open"}>
        {draft === undefined ? (
          <span className="muted">No draft is open.</span>
        ) : (
          <>
            <Badge tone="warn">Draft open</Badge>
            <span className="muted">{draft.extends == null ? "A first version, not yet published" : `Extends version ${String(draft.extends)}`}</span>
            <Link to={routeCanvasPath(id, undefined, DEFAULT_VIEW)}>Canvas</Link>
          </>
        )}
      </span>
      <span className="row">
        {moves.open ? <Button disabled={write.disabled} onClick={() => { send(openDraft()); }}>Open a draft</Button> : null}
        {moves.publish ? <Button primary disabled={write.disabled} onClick={() => { send({ op: "publish_draft" }); }}>Publish the draft</Button> : null}
        {moves.discard ? <Button disabled={write.disabled} onClick={() => { send({ op: "discard_draft" }); }}>Discard the draft</Button> : null}
        {draft === undefined ? null : <Button onClick={() => void exportDraft(undefined)}>Export the draft</Button>}
        <Button disabled={write.disabled} onClick={() => { send(retireMutation(moves.retire)); }}>
          {moves.retire ? "Retire" : "Bring back"}
        </Button>
      </span>
      {write.rejected === undefined ? null : (
        <Refused rejection={write.rejected} onDismiss={write.dismiss}>
          {violates(write.rejected, "draft_exists") ? <Button onClick={() => { send({ op: "discard_draft" }); }}>Discard the open draft</Button> : null}
        </Refused>
      )}
      <ImportFile label="Import a file as a new draft" />
    </section>
  );
}
