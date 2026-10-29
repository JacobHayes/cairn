// A11, A12, A13: the new-route form (`/routes/new`): a route started empty, to
// author by hand (one route patch at revision 0 that creates it and opens its
// first draft, then its draft's canvas), or a route file imported. A new one's id is a slug from
// its name, editable (folded) before it is made, and unique in the deployment (the engine refuses
// one in use) and not `new`, which the form's own address shadows.
import { useId } from "react";
import { Link, useNavigate } from "react-router";

import { useValueProblems } from "../authoring/form-problems.ts";
import { slugOf } from "../authoring/keys.ts";
import { CheckFailed, invalidAttrs, Problem } from "../authoring/parts.tsx";
import { useDraft, writeDraft } from "../data/drafts.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Input } from "../ui/kit.tsx";
import { ROUTES_INDEX_PATH, routePath } from "./address.ts";
import { ImportFile } from "./ImportFile.tsx";
import { openDraft } from "./model.ts";

/** The form: the name, and its id (a slug from the name) folded away until it needs changing. */
function NewForm({ disabled, refused, onStart }: { disabled: boolean; refused: boolean; onStart: (name: string, id: string) => void }) {
  const [kept, setKept] = useDraft<{ name: string; id?: string }>("new-route");
  const name = kept?.name ?? "";
  const routeId = kept?.id ?? slugOf(name, "route");
  const checks = useValueProblems([
    { field: "name", kind: "title", what: "The name", value: name.trim(), optional: true },
    { field: "id", kind: "route_id", what: "An id", value: routeId, optional: name.trim() === "" },
  ]);
  const [nameMessage, idMessage] = [useId(), useId()];
  const nameProblem = checks.problems.name;
  const idProblem = checks.problems.id;
  return (
    <form className="stack" onSubmit={(event) => { event.preventDefault(); onStart(name.trim(), routeId); }}>
      <label className="stack">
        <span className="label">Name</span>
        <Input className="new-field" aria-label="New route name" {...invalidAttrs(nameMessage, nameProblem)} placeholder="Name of the route" autoFocus value={name} onChange={(event) => { setKept({ ...kept, name: event.target.value }); }} />
      </label>
      <details key={String(refused)} open={refused || idProblem !== undefined}>
        <summary className="muted">Change id</summary>
        <label className="stack">
          <span className="label">Id</span>
          <Input className="new-field" aria-label="New route id" {...(idProblem === undefined ? { "aria-invalid": refused } : invalidAttrs(idMessage, idProblem))} value={routeId} onChange={(event) => { setKept({ name, id: event.target.value }); }} />
        </label>
      </details>
      {nameProblem === undefined ? null : <Problem id={nameMessage}>{nameProblem}</Problem>}
      {idProblem === undefined ? null : <Problem id={idMessage}>{idProblem}</Problem>}
      <CheckFailed failure={checks.failure} retry={checks.retry} />
      <Button type="submit" primary disabled={disabled || name.trim() === "" || !checks.checked || nameProblem !== undefined || idProblem !== undefined} data-testid="new-create">
        Create the route
      </Button>
    </form>
  );
}

export function NewRoutePage() {
  const write = useScreenWrite("new-route");
  const navigate = useNavigate();
  const start = async (name: string, routeId: string) => {
    const landed = await write.run({ target: { route: routeId }, baseRevision: 0, mutations: [{ op: "create_route", name }, openDraft()] });
    if (landed) {
      writeDraft("new-route", undefined);
      void navigate(routePath(routeId));
    }
  };
  return (
    <section className="stack new-route" aria-label="New route" data-testid="new-route">
      <div className="row">
        <h1>New route</h1>
        <span className="spacer" />
        <Link to={ROUTES_INDEX_PATH}>All routes</Link>
      </div>
      <NewForm disabled={write.disabled} refused={write.rejection?.rejection === "stale"} onStart={(name, id) => { void start(name, id); }} />
      {write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} staleHeading="a route with this id already exists. Pick another id." />}
      <h2>Or import a route file</h2>
      <ImportFile edit="import:new-route" label="Import a file…" />
    </section>
  );
}
