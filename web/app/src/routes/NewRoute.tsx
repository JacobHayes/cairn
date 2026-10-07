// A11, A12: a new route started empty, to author by hand: one route patch at revision 0 that
// creates it and opens its first draft, then its draft's canvas. Its id is a slug from its
// name, editable before it is made, and unique in the deployment (the engine refuses one in
// use).
import { useNavigate } from "react-router";

import { isSlug, slugOf } from "../authoring/keys.ts";
import { useDraft } from "../data/drafts.ts";
import { TITLE_BYTES_MAX, overBytes } from "../authoring/limits.ts";
import { DEFAULT_VIEW } from "../canvas/settings.ts";
import { Refused } from "../screens/Refused.tsx";
import { routeCanvasPath } from "../screens/RouteCanvasPage.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Field } from "../ui/kit.tsx";
import { openDraft } from "./model.ts";

export function NewRoute() {
  const write = useScreenWrite();
  const navigate = useNavigate();
  const [kept, setKept] = useDraft<{ name: string; id?: string }>("new-route");
  const name = kept?.name ?? "";
  const id = kept?.id;
  const setName = (next: string) => {
    setKept({ ...kept, name: next });
  };
  const setId = (next: string) => {
    setKept({ name, id: next });
  };
  const routeId = id ?? slugOf(name, "route");
  const problem = name.trim() === "" ? undefined : (overBytes(name, TITLE_BYTES_MAX, "The name") ?? (isSlug(routeId) ? undefined : "An id is lowercase letters, digits, '_' and '-'."));
  const start = async () => {
    const landed = await write.run({ target: { route: routeId }, baseRevision: 0, mutations: [{ op: "create_route", name: name.trim() }, openDraft()] });
    if (landed) {
      setKept(undefined);
      void navigate(routeCanvasPath(routeId, undefined, DEFAULT_VIEW));
    }
  };
  return (
    <form className="stack" data-testid="new-route" onSubmit={(event) => { event.preventDefault(); void start(); }}>
      <span className="row">
        <Field aria-label="New route name" placeholder="A new route's name" value={name} onChange={(event) => { setName(event.target.value); }} />
        <Field aria-label="New route id" value={routeId} onChange={(event) => { setId(event.target.value); }} />
        <Button type="submit" primary disabled={write.disabled || name.trim() === "" || problem !== undefined}>
          Start a route with an empty draft
        </Button>
      </span>
      {problem === undefined ? null : <span className="author-problem" role="alert">{problem}</span>}
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
    </form>
  );
}
