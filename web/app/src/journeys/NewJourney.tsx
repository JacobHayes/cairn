// B1: starting a journey, from a route's version (the latest published by default) or empty,
// with a name and an optional description. It is the journey's first patch (base revision 0,
// A17) under an id made from its name; once it lands the journey opens on its decision
// walkthrough (C11). Retired routes are not offered (A19). The form is a draft, so a reload
// keeps it (ARCHITECTURE, Web UI).
import { useLocation, useNavigate } from "react-router";

import { walkthroughPath } from "../acting/address.ts";
import { useDraft } from "../data/drafts.ts";
import { routeIndex } from "../data/reads.ts";
import { useLive } from "../data/react.ts";
import { Refused } from "../screens/Refused.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Field, Panel } from "../ui/kit.tsx";
import { createMutation, newJourneyId, startableRoutes, type RouteSummary } from "./lifecycle.ts";
import "./journeys.css";

interface Form {
  name: string;
  description: string;
  /** The route's id, or "" for an empty journey. */
  route: string;
  /** The version, or 0 for the route's latest. */
  version: number;
}

function StartFrom({ form, routes, onChange }: { form: Form; routes: RouteSummary[]; onChange: (form: Form) => void }) {
  const chosen = routes.find((route) => route.header.id === form.route);
  const latest = chosen?.latest_version ?? 0;
  return (
    <>
      <label htmlFor="new-route">Start from</label>
      <select id="new-route" className="select" value={form.route} onChange={(event) => { onChange({ ...form, route: event.target.value, version: 0 }); }}>
        <option value="">An empty journey</option>
        {form.route !== "" && chosen === undefined ? <option value={form.route}>{form.route} (not offered)</option> : null}
        {routes.map((route) => (
          <option key={route.header.id} value={route.header.id}>
            {route.header.name}
          </option>
        ))}
      </select>
      {chosen === undefined ? null : (
        <>
          <label htmlFor="new-version">Version</label>
          <select id="new-version" className="select" value={form.version === 0 ? latest : form.version} onChange={(event) => { onChange({ ...form, version: Number(event.target.value) }); }}>
            {Array.from({ length: latest }, (_, at) => latest - at).map((version) => (
              <option key={version} value={version}>
                {version === latest ? `${String(version)} (latest)` : version}
              </option>
            ))}
          </select>
        </>
      )}
    </>
  );
}

function NewJourneyForm({ routes, initial, asked }: { routes: RouteSummary[]; initial: Form; asked: string }) {
  const navigate = useNavigate();
  const write = useScreenWrite();
  const [draft, setDraft] = useDraft<Form>(`new-journey:${asked}`);
  const form = draft ?? initial;
  const chosen = routes.find((route) => route.header.id === form.route);
  // A route chosen, or kept in the draft, that is no longer offered (retired meanwhile) is not
  // "empty": the person must choose again rather than start a journey with no lineage.
  const unavailable = form.route !== "" && chosen === undefined;
  const start = async () => {
    if (unavailable) {
      return;
    }
    const id = newJourneyId(form.name);
    const version = form.version === 0 ? (chosen?.latest_version ?? 0) : form.version;
    const from = chosen === undefined ? undefined : { route: chosen.header.id, version };
    if (await write.run({ target: { journey: id }, baseRevision: 0, mutations: [createMutation(form.name, form.description, from)] })) {
      setDraft(undefined);
      void navigate(walkthroughPath(id));
    }
  };
  return (
    <form className="stack" data-testid="new-journey-form" onSubmit={(event) => { event.preventDefault(); void start(); }}>
      <div className="form-grid">
        <label htmlFor="new-name">Name</label>
        <Field id="new-name" required value={form.name} onChange={(event) => { setDraft({ ...form, name: event.target.value }); }} />
        <label htmlFor="new-description">Description</label>
        <textarea id="new-description" className="textarea" value={form.description} onChange={(event) => { setDraft({ ...form, description: event.target.value }); }} />
        <StartFrom form={form} routes={routes} onChange={setDraft} />
      </div>
      {unavailable ? (
        <p className="callout" data-testid="route-unavailable">
          The route chosen is retired or has no published version now; choose where to start again.
        </p>
      ) : null}
      <span className="row">
        <Button primary type="submit" disabled={write.disabled || unavailable || form.name.trim() === ""}>Start the journey</Button>
      </span>
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
    </form>
  );
}

export function NewJourney() {
  const { search } = useLocation();
  const params = new URLSearchParams(search);
  const { view } = useLive("routes", routeIndex);
  if (view.status !== "ready") {
    return <p className="muted">{view.status === "failed" ? `The routes could not be read: ${view.message}` : "Loading the routes..."}</p>;
  }
  const routes = startableRoutes(view.value);
  const asked = params.get("route") ?? "";
  const offered = routes.some((route) => route.header.id === asked);
  const initial: Form = { name: "", description: "", route: asked, version: offered ? Number(params.get("version") ?? 0) || 0 : 0 };
  return (
    <Panel aria-label="New journey">
      <h1 className="title">New journey</h1>
      <NewJourneyForm key={search} routes={routes} initial={initial} asked={search} />
    </Panel>
  );
}
