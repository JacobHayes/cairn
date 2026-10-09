// A13: importing a route file: a new route when no route has its id, or a new draft of that
// route extending its latest version. The file is read by the page's engine (YAML on disk,
// JSON on the wire) and imported as one route patch under a fresh patch id (H5). A11: while
// another draft is open the import is refused, with the option to discard that draft.
import { useEffect, useId, useState } from "react";
import { useNavigate } from "react-router";

import type { Schema } from "@cairn/client";

import type { RouteFile } from "../data/host.ts";
import { consequenceLines, noticesOf } from "../data/activity.ts";
import { unlandedOf, useProblem, useSession, useSkew } from "../data/react.ts";
import { newPatchId, type Rejection } from "../data/writes.ts";
import { Refused, violates } from "../screens/Refused.tsx";
import { Button } from "../ui/kit.tsx";
import { routeDetailPath } from "./address.ts";

/** What a screen does once a file is imported, in place of going to the route's detail: A20, the notices the import left. */
export type ImportedHandler = (file: RouteFile, notices: Schema<"Notice">[]) => void;

type Outcome = { status: "idle" } | { status: "unreadable"; message: string } | { status: "rejected"; file: RouteFile; rejection: Rejection };

function useImport(onImported: ImportedHandler | undefined) {
  const session = useSession();
  const navigate = useNavigate();
  const [outcome, setOutcome] = useState<Outcome>({ status: "idle" });
  const [pending, setPending] = useState(false);
  const dismiss = () => {
    setOutcome({ status: "idle" });
  };
  const key = `import:${useId()}`;
  const report = useProblem(
    key,
    outcome.status === "idle" ? undefined : outcome.status === "rejected" ? unlandedOf(outcome.rejection) : { kind: "failed", message: `The file could not be imported: ${outcome.message}` },
    { discard: dismiss },
  );
  useEffect(() => () => { session.sync.resolve(key); }, [session, key]);
  const send = async (file: RouteFile) => {
    // ARCHITECTURE, Web UI: version skew. A file read while the tab latched skew is not sent.
    if (session.skew.latched) {
      return;
    }
    setPending(true);
    const answered = await session.sync.track(session.host.importRoute({ patch_id: newPatchId(), file }));
    setPending(false);
    if (answered.outcome === "answered") {
      setOutcome({ status: "idle" });
      session.recordSave(`Imported ${file.name} as a draft of ${file.route}`, consequenceLines(answered.answer));
      if (onImported === undefined) {
        void navigate(routeDetailPath(file.route));
      } else {
        onImported(file, noticesOf(answered.answer));
      }
    } else if (answered.outcome === "rejected") {
      setOutcome({ status: "rejected", file, rejection: answered.rejection });
      report(unlandedOf(answered.rejection));
    } else {
      setOutcome({ status: "unreadable", message: answered.error.message });
      report({ kind: "failed", message: `The file could not be imported: ${answered.error.message}` });
    }
  };
  const choose = async (chosen: File) => {
    let file: RouteFile;
    try {
      file = session.host.files.read(await chosen.text());
    } catch (thrown) {
      setOutcome({ status: "unreadable", message: thrown instanceof Error ? thrown.message : String(thrown) });
      return;
    }
    await send(file);
  };
  /** A11: discards the route's open draft, then imports the file again. */
  const discardAndImport = async (file: RouteFile) => {
    const route = await session.host.route(file.route);
    const discarded = await session.write({ target: { route: file.route }, baseRevision: route.revision, mutations: [{ op: "discard_draft" }] });
    if (discarded.outcome === "landed") {
      await send(file);
    }
  };
  return { outcome, pending, choose, discardAndImport, dismiss };
}

export function ImportFile({ label = "Import a route file", onImported }: { label?: string; onImported?: ImportedHandler }) {
  const skew = useSkew();
  const { outcome, pending, choose, discardAndImport, dismiss } = useImport(onImported);
  return (
    <div className="stack" data-testid="import">
      <label className="button import-button">
        {label}
        <input
          className="visually-hidden"
          type="file"
          accept=".yaml,.yml,.json"
          aria-label={label}
          disabled={pending || skew !== undefined}
          onChange={(event) => {
            const chosen = event.target.files?.[0];
            event.target.value = "";
            if (chosen !== undefined) {
              void choose(chosen);
            }
          }}
        />
      </label>
      {outcome.status === "unreadable" ? <p className="callout callout-bad" role="alert">The file could not be imported: {outcome.message}</p> : null}
      {outcome.status === "rejected" ? (
        <Refused rejection={outcome.rejection} onDismiss={dismiss}>
          {violates(outcome.rejection, "draft_exists") ? <Button onClick={() => void discardAndImport(outcome.file)}>Discard the open draft and import</Button> : null}
        </Refused>
      ) : null}
    </div>
  );
}
