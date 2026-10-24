// B1: starting a journey, from a route's version (the latest published by default) or empty,
// with a name, an optional description and owners. It is the journey's first patch (base revision 0,
// A17) under an id made from its name; once it lands the journey opens on its decision
// walkthrough (C11). Retired routes are not offered (A19). The form is a draft, so a reload
// keeps it (ARCHITECTURE, Web UI).
import { useLocation, useNavigate } from "react-router";

import { useDraft } from "../data/drafts.ts";
import { routeIndex } from "../data/reads.ts";
import { valueOf } from "../data/keyed-read.ts";
import { useDeployment, useLive } from "../data/react.ts";
import { startableVersion } from "../routes/model.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Input, Panel } from "../ui/kit.tsx";
import { Failed } from "../ui/Failed.tsx";
import { OwnersField } from "./OwnersField.tsx";
import { walkthroughPath } from "./view.ts";
import { createMutation, newJourneyId, type RouteSummary } from "./lifecycle.ts";

interface Form {
  name: string;
  description: string;
  /** The identities who own it, by key. */
  owners: string[];
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
      <select id="new-route" value={form.route} onChange={(event) => { onChange({ ...form, route: event.target.value, version: 0 }); }}>
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
          <select id="new-version" value={form.version === 0 ? latest : form.version} onChange={(event) => { onChange({ ...form, version: Number(event.target.value) }); }}>
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
  const write = useScreenWrite("new-journey");
  const [draft, setDraft] = useDraft<Form>(`new-journey:${asked}`);
  // A draft kept before owners were chosen here has none.
  const stored: Partial<Form> = draft ?? initial;
  const form: Form = { ...(draft ?? initial), owners: stored.owners ?? [] };
  const deployment = useDeployment();
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
    if (await write.run({ target: { journey: id }, baseRevision: 0, ...(deployment === undefined ? {} : { deploymentRevision: deployment.revision }), mutations: [createMutation(form.name, form.description, form.owners, from)] })) {
      setDraft(undefined);
      void navigate(walkthroughPath(id));
    }
  };
  return (
    <form className="stack" data-testid="new-journey-form" onSubmit={(event) => { event.preventDefault(); void start(); }}>
      <div className="form-grid">
        <label htmlFor="new-name">Name</label>
        <Input id="new-name" required value={form.name} onChange={(event) => { setDraft({ ...form, name: event.target.value }); }} />
        <label htmlFor="new-description">Description</label>
        <textarea id="new-description" value={form.description} onChange={(event) => { setDraft({ ...form, description: event.target.value }); }} />
        <OwnersField owners={form.owners} onChange={(owners) => { setDraft({ ...form, owners }); }} />
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
      {write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />}
    </form>
  );
}

export function NewJourney() {
  const { search } = useLocation();
  const params = new URLSearchParams(search);
  const { view } = useLive("routes", routeIndex);
  const index = valueOf(view);
  if (index === undefined) {
    return <p className="muted small">{view.status === "failed" ? <Failed what="The routes" error={view.error} /> : "Reading the routes..."}</p>;
  }
  const routes = index.filter((route) => startableVersion(route) !== undefined);
  const asked = params.get("route") ?? "";
  const offered = routes.some((route) => route.header.id === asked);
  const initial: Form = { name: "", description: "", owners: [], route: asked, version: offered ? Number(params.get("version") ?? 0) || 0 : 0 };
  return (
    <Panel aria-label="New journey">
      <h1>New journey</h1>
      <NewJourneyForm key={search} routes={routes} initial={initial} asked={search} />
    </Panel>
  );
}
