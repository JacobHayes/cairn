// A11, A12, A21, A13: the Library's `New` menu: a route or segment started empty, to author by
// hand (one route patch at revision 0 that creates it, with its kind, and opens its first
// draft, then its draft's canvas), or a route file imported. A new one's id is a slug from its
// name, editable (folded) before it is made, and unique in the deployment (the engine refuses
// one in use).
import type { Schema } from "@cairn/client";
import { useState } from "react";
import { useNavigate } from "react-router";

import { isSlug, slugOf } from "../authoring/keys.ts";
import { useDraft, writeDraft } from "../data/drafts.ts";
import { TITLE_BYTES_MAX, overBytes } from "../authoring/limits.ts";
import { DEFAULT_VIEW } from "../canvas/settings.ts";
import { Refused } from "../screens/Refused.tsx";
import { Menu } from "../screens/Menu.tsx";
import { routeCanvasPath } from "../screens/RouteCanvasPage.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Field } from "../ui/kit.tsx";
import { routeDetailPath } from "./address.ts";
import { ImportFile } from "./ImportFile.tsx";
import { openDraft } from "./model.ts";

type Kind = Schema<"RouteKind">;

/** The form a kind opens: its name, and its id (a slug from the name) folded away until it needs changing. */
function NewForm({ kind, disabled, onStart }: { kind: Kind; disabled: boolean; onStart: (name: string, id: string) => void }) {
  const [kept, setKept] = useDraft<{ name: string; id?: string }>("new-route");
  const name = kept?.name ?? "";
  const routeId = kept?.id ?? slugOf(name, "route");
  const problem = name.trim() === "" ? undefined : (overBytes(name, TITLE_BYTES_MAX, "The name") ?? (isSlug(routeId) ? undefined : "An id is lowercase letters, digits, '_' and '-'."));
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

/** What `New` offers: a route or a segment to start empty, and a file to import. */
function NewChoices({ disabled, onStart, close }: { disabled: boolean; onStart: (kind: Kind, name: string, id: string) => void; close: () => void }) {
  const [kind, setKind] = useState<Kind | undefined>(undefined);
  const navigate = useNavigate();
  if (kind !== undefined) {
    return <NewForm kind={kind} disabled={disabled} onStart={(name, id) => { close(); onStart(kind, name, id); }} />;
  }
  return (
    <>
      {(["process", "segment"] as const).map((each) => (
        <button key={each} type="button" role="menuitem" className="menu-item" data-testid={`new-${each}`} onClick={() => { setKind(each); }}>
          {each === "process" ? "Route" : "Segment"}
        </button>
      ))}
      <ImportFile label="Import a file…" onImported={(file) => { close(); void navigate(routeDetailPath(file.route)); }} />
    </>
  );
}

export function NewRoute() {
  const write = useScreenWrite();
  const navigate = useNavigate();
  const start = async (kind: Kind, name: string, routeId: string) => {
    const landed = await write.run({ target: { route: routeId }, baseRevision: 0, mutations: [{ op: "create_route", name, kind }, openDraft()] });
    if (landed) {
      writeDraft("new-route", undefined);
      void navigate(routeCanvasPath(routeId, undefined, DEFAULT_VIEW));
    }
  };
  return (
    <div className="stack" data-testid="new-route">
      <span className="row">
        <Menu label="New" testId="new-menu" trigger="New ▾" role="dialog">
          {(close) => <NewChoices disabled={write.disabled} onStart={(kind, name, id) => { void start(kind, name, id); }} close={close} />}
        </Menu>
      </span>
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
    </div>
  );
}
