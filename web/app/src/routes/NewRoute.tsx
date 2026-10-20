// A11, A12, A21, A13: the new-route form (`/routes/new`): a route or segment started empty, to
// author by hand (one route patch at revision 0 that creates it, with its kind, and opens its
// first draft, then its draft's canvas), or a route file imported. A new one's id is a slug from
// its name, editable (folded) before it is made, and unique in the deployment (the engine refuses
// one in use) and not `new`, which the form's own address shadows.
import type { Schema } from "@cairn/client";
import { Link, useNavigate, useSearchParams } from "react-router";

import { isSlug, slugOf } from "../authoring/keys.ts";
import { useDraft, writeDraft } from "../data/drafts.ts";
import { TITLE_BYTES_MAX, overBytes } from "../authoring/limits.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Field, Segmented } from "../ui/kit.tsx";
import { ROUTES_INDEX_PATH, routePath } from "./address.ts";
import { ImportFile } from "./ImportFile.tsx";
import { openDraft } from "./model.ts";

type Kind = Schema<"RouteKind">;

/** The id the form refuses: `/routes/new` is the form, so a route called that would have no canvas address. */
const RESERVED_ID = "new";

/** The form a kind opens: its name, and its id (a slug from the name) folded away until it needs changing. */
function NewForm({ kind, disabled, refused, onStart }: { kind: Kind; disabled: boolean; refused: boolean; onStart: (name: string, id: string) => void }) {
  const [kept, setKept] = useDraft<{ name: string; id?: string }>("new-route");
  const name = kept?.name ?? "";
  const routeId = kept?.id ?? slugOf(name, "route");
  const problem = name.trim() === "" ? undefined : (overBytes(name, TITLE_BYTES_MAX, "The name") ?? (isSlug(routeId) && routeId !== RESERVED_ID ? undefined : "An id is lowercase letters, digits, '_' and '-', and not 'new'."));
  const word = kind === "segment" ? "segment" : "route";
  return (
    <form className="stack" onSubmit={(event) => { event.preventDefault(); onStart(name.trim(), routeId); }}>
      <label className="stack">
        <span className="label">Name</span>
        <Field className="new-field" aria-label="New route name" placeholder={`Name of the ${word}`} autoFocus value={name} onChange={(event) => { setKept({ ...kept, name: event.target.value }); }} />
      </label>
      <details key={String(refused)} open={refused}>
        <summary className="muted">Change id</summary>
        <label className="stack">
          <span className="label">Id</span>
          <Field className="new-field" aria-label="New route id" aria-invalid={refused} value={routeId} onChange={(event) => { setKept({ name, id: event.target.value }); }} />
        </label>
      </details>
      {problem === undefined ? null : <span className="author-problem" role="alert">{problem}</span>}
      <Button type="submit" primary disabled={disabled || name.trim() === "" || problem !== undefined} data-testid="new-create">
        Create the {word}
      </Button>
    </form>
  );
}

/** The kind choice: picking one replaces `kind` in the address (DESIGN, Segmented control). */
function KindChoice({ kind }: { kind: Kind }) {
  const [, setParams] = useSearchParams();
  return (
    <Segmented
      label="Kind"
      value={kind}
      options={[{ value: "process", label: "Route" }, { value: "segment", label: "Segment" }]}
      onChange={(next) => { setParams(next === "segment" ? { kind: "segment" } : {}, { replace: true }); }}
      data-testid="new-route-kinds"
    />
  );
}

export function NewRoutePage() {
  const write = useScreenWrite("new-route");
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const kind: Kind = params.get("kind") === "segment" ? "segment" : "process";
  const start = async (name: string, routeId: string) => {
    const landed = await write.run({ target: { route: routeId }, baseRevision: 0, mutations: [{ op: "create_route", name, kind }, openDraft()] });
    if (landed) {
      writeDraft("new-route", undefined);
      void navigate(routePath(routeId));
    }
  };
  return (
    <section className="stack new-route" aria-label="New route" data-testid="new-route">
      <div className="row">
        <h1>{kind === "segment" ? "New segment" : "New route"}</h1>
        <span className="spacer" />
        <Link to={ROUTES_INDEX_PATH}>All routes</Link>
      </div>
      <KindChoice kind={kind} />
      <NewForm kind={kind} disabled={write.disabled} refused={write.rejection?.rejection === "stale"} onStart={(name, id) => { void start(name, id); }} />
      {write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} staleHeading="a route with this id already exists. Pick another id." />}
      <h2>Or import a route file</h2>
      <ImportFile edit="import:new-route" label="Import a file…" />
    </section>
  );
}
