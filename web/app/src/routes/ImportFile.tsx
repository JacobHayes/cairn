// A13: importing a route file: a new route when no route has its id, or a new draft of that
// route extending its latest version. The file is read by the page's engine (YAML on disk,
// JSON on the wire) and imported as one route write under a fresh patch id (H5). A11: while
// another draft is open the import is refused, with the option to discard that draft. The
// reading, the discarding and the importing are a workflow (import.ts) over the edit manager's
// writes: the control reads its writes by its edit key, so a refusal stays with its file across
// leaving the screen and a reload.
import { useActorRef, useSelector } from "@xstate/react";
import type { ActorRefFrom } from "xstate";
import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router";

import type { Schema } from "@cairn/client";

import type { RouteFile } from "../data/host.ts";
import { noticesOf } from "../data/activity.ts";
import { useEdit, useSession, useSkew } from "../data/react.ts";
import { isPatchAnswer, violates } from "../data/writes.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Button } from "../ui/kit.tsx";
import { routePath } from "./address.ts";
import { importFlow } from "./import.ts";

/** What a screen does once a file is imported, in place of going to the route's page: A20, the notices the import left. */
export type ImportedHandler = (file: RouteFile, notices: Schema<"Notice">[]) => void;

/** The import workflow, owned by whoever calls this: keep it above anything that remounts while an import is under way. */
export type ImportFlow = ActorRefFrom<typeof importFlow>;

/**
 * An import workflow. `edit` is its own key (never a sibling control's): its import write is
 * kept under it and its discard of an open draft under `${edit}:discard`.
 */
export function useImportFlow(edit: string): ImportFlow {
  const session = useSession();
  return useActorRef(importFlow, {
    input: {
      edits: session.edits,
      keys: { import: edit, discard: `${edit}:discard` },
      read: (text) => session.host.files.read(text),
      route: (id) => session.host.route(id),
    },
  });
}

/** Told of a landing only while the caller is on screen: a person who left is not taken anywhere. */
export function useOnImported(flow: ImportFlow, onImported: ImportedHandler): void {
  useEffect(() => {
    const subscription = flow.on("imported", ({ file, answer }) => {
      onImported(file, isPatchAnswer(answer) ? noticesOf(answer) : []);
    });
    return () => {
      subscription.unsubscribe();
    };
  }, [flow, onImported]);
}

/** An import control with its own workflow: on a landing it goes to the route's page, or tells `onImported`. */
export function ImportFile({ edit, label = "Import a route file", onImported }: { edit: string; label?: string; onImported?: ImportedHandler }) {
  const navigate = useNavigate();
  const flow = useImportFlow(edit);
  useOnImported(
    flow,
    useCallback(
      (file, notices) => {
        if (onImported === undefined) {
          void navigate(routePath(file.route));
        } else {
          onImported(file, notices);
        }
      },
      [navigate, onImported],
    ),
  );
  return <ImportControl flow={flow} edit={edit} label={label} />;
}

/** The control over `flow`: the file chooser, the refusal that stays with its file across a reload, and the discard's state. */
export function ImportControl({ flow, edit, label = "Import a route file" }: { flow: ImportFlow; edit: string; label?: string }) {
  const skew = useSkew();
  const working = useSelector(flow, (snapshot) => snapshot.hasTag("busy"));
  const unreadable = useSelector(flow, (snapshot) => snapshot.context.unreadable);
  const importing = useEdit(edit);
  const discarding = useEdit(`${edit}:discard`);
  // The file lives in the refused import write, so importing over the open draft works after a reload.
  const refused = importing.rejection === undefined || importing.write?.command.kind !== "import" ? undefined : importing.write.command.request.file;
  return (
    <div className="stack" data-testid="import">
      <label className="button import-button">
        {label}
        <input
          className="visually-hidden"
          type="file"
          accept=".yaml,.yml,.json"
          aria-label={label}
          disabled={working || importing.busy || discarding.busy || skew}
          onChange={(event) => {
            const chosen = event.target.files?.[0];
            event.target.value = "";
            if (chosen !== undefined) {
              flow.send({ type: "choose", file: chosen });
            }
          }}
        />
      </label>
      {unreadable === undefined ? null : <p className="callout callout-bad" role="alert">{unreadable}</p>}
      {importing.rejection === undefined ? (
        <WriteStatus view={importing.status} />
      ) : (
        <RejectionView rejection={importing.rejection} onDismiss={importing.dismiss}>
          {violates(importing.rejection, "draft_exists") && refused !== undefined ? (
            <Button disabled={working || discarding.busy || skew} onClick={() => { flow.send({ type: "replace", file: refused }); }}>
              Discard the open draft and import
            </Button>
          ) : null}
        </RejectionView>
      )}
      {discarding.rejection === undefined ? <WriteStatus view={discarding.status} /> : <RejectionView rejection={discarding.rejection} onDismiss={discarding.dismiss} />}
    </div>
  );
}
