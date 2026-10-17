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
import { DEFAULT_VIEW } from "../canvas/settings.ts";
import { Refused } from "../screens/Refused.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Field } from "../ui/kit.tsx";
import { ROUTES_INDEX_PATH, routeCanvasPath } from "./address.ts";
import { ImportFile } from "./ImportFile.tsx";
import { openDraft } from "./model.ts";

type Kind = Schema<"RouteKind">;

/** The id the form refuses: `/routes/new` is the form, so a route called that would have no canvas address. */
const RESERVED_ID = "new";

/** The form a kind opens: its name, and its id (a slug from the name) folded away until it needs changing. */
function NewForm({ kind, disabled, onStart }: { kind: Kind; disabled: boolean; onStart: (name: string, id: string) => void }) {
  const [kept, setKept] = useDraft<{ name: string; id?: string }>("new-route");
  const name = kept?.name ?? "";
  const routeId = kept?.id ?? slugOf(name, "route");
  const problem = name.trim() === "" ? undefined : (overBytes(name, TITLE_BYTES_MAX, "The name") ?? (isSlug(routeId) && routeId !== RESERVED_ID ? undefined : "An id is lowercase letters, digits, '_' and '-', and not 'new'."));
  const word = kind === "segment" ? "segment" : "route";
  return (
    <form className="stack" onSubmit={(event) => { event.preventDefault(); onStart(name.trim(), routeId); }}>
      <Field aria-label="New route name" placeholder={`Name of the ${word}`} autoFocus value={name} onChange={(event) => { setKept({ ...kept, name: event.target.value }); }} />
      <details className="small">
        <summary className="muted">Change id</summary>
        <Field aria-label="New route id" value={routeId} onChange={(event) => { setKept({ name, id: event.target.value }); }} />
      </details>
      {problem === undefined ? null : <span className="author-problem" role="alert">{problem}</span>}
      <Button type="submit" primary disabled={disabled || name.trim() === "" || problem !== undefined} data-testid="new-create">
        Create the {word}
      </Button>
    </form>
  );
}

/** The kind choice, as the index's type chips: picking one replaces `kind` in the address. */
function KindChips({ kind }: { kind: Kind }) {
  const [, setParams] = useSearchParams();
  return (
    <div className="journey-chips" role="group" aria-label="Kind" data-testid="new-route-kinds">
      {(["process", "segment"] as const).map((each) => (
        <button key={each} type="button" className="chip" aria-pressed={kind === each} data-testid={`new-${each}`} onClick={() => { setParams(each === "segment" ? { kind: "segment" } : {}, { replace: true }); }}>
          {each === "process" ? "Route" : "Segment"}
        </button>
      ))}
    </div>
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
      void navigate(routeCanvasPath(routeId, undefined, DEFAULT_VIEW));
    }
  };
  return (
    <section className="stack" aria-label="New route" data-testid="new-route">
      <div className="row">
        <h1>{kind === "segment" ? "New segment" : "New route"}</h1>
        <span className="spacer" />
        <Link to={ROUTES_INDEX_PATH}>All routes</Link>
      </div>
      <KindChips kind={kind} />
      <NewForm kind={kind} disabled={write.disabled} onStart={(name, id) => { void start(name, id); }} />
      {write.rejected === undefined ? <WriteStatus view={write.status} /> : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
      <h2>Or import a route file</h2>
      <ImportFile edit="import:new-route" label="Import a file…" />
    </section>
  );
}
