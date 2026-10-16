// Authoring's write path: every edit is previewed with the wasm engine before it is sent
// (ARCHITECTURE, Web UI: Previews), so a rejection shows at the field without a round trip,
// then sent through the shell's write path (4.6: H5's safe retry, D7's notice, nothing sent
// under version skew). A journey's patch is applied to the document the derive worker holds;
// a route's to the route as read. Committed state always comes from the host. The preview of
// what a form would send also runs as the author types, so its fields show what the engine
// would say before Save.
import type { Schema } from "@cairn/client";
import { HostFailure } from "@cairn/wasm";
import { useEffect, useId, useState } from "react";

import type { Deriver, Host } from "../data/host.ts";
import { Skewed } from "../data/host.ts";
import { beforeWork, DERIVE, previewPolicy, type ReadAt, type ReadKey, type ReadPolicy, type ReadSource } from "../data/keyed-read.ts";
import { unlandedOf, useKeyedRead, useProblem, useSession, useSkew, useViewer } from "../data/react.ts";
import { useReceipt, type ReceiptState } from "../ui/Receipt.tsx";
import { newPatchId, type Rejection } from "../data/writes.ts";
import { writesEntities } from "../detail/write.ts";
import type { Mutation } from "./graph.ts";
import type { Authored } from "./target.ts";
import type { Place } from "./violations.ts";

/** What the engine says of a patch before it is sent. */
export type Preview =
  | { outcome: "accepted"; consequences: Schema<"Consequences"> | undefined }
  | { outcome: "rejected"; rejection: Rejection }
  | { outcome: "unavailable"; message: string };

/** A patch and the form places its mutations came from, by position (A15). */
export interface Sent {
  mutations: Mutation[];
  places: Place[];
}

export interface AuthorWrite {
  /**
   * Previews `mutations` on the graph as held, then sends them drafted against `base` (the
   * revision their author started from, H5; the one held by default) unless the preview
   * rejects them; true when they landed.
   */
  run: (mutations: Mutation[], places?: Place[], base?: number) => Promise<boolean>;
  pending: boolean;
  /** Version skew or a write in flight: no new write starts. */
  disabled: boolean;
  /** The last attempt's rejection (the preview's or the host's), until dismissed or a later attempt. */
  failed: { sent: Sent; rejection: Rejection } | undefined;
  /**
   * The last attempt got no answer (`failed`: it may or may not have been saved), was not sent
   * (`stopped`: the app was updated), or found what it names removed (`gone`), until a later attempt.
   */
  unanswered: "failed" | "stopped" | "gone" | undefined;
  dismiss: () => void;
  /** What the last landed write leaves under the form, for a few seconds. */
  receipt: ReceiptState | undefined;
}

/** Who a preview is made as when the caller is not known yet. */
export const LOCAL_USER = "u_local";

/** A patch of `mutations` for `authored`'s target, drafted against `base`. */
export function authoredPatchOf(authored: Authored, mutations: Mutation[], base = authored.revision): Schema<"Patch"> {
  const patch: Schema<"Patch"> = { id: newPatchId(), target: authored.target, base_revision: base, mutations };
  return "journey" in authored.target && writesEntities(mutations) ? { ...patch, deployment_revision: authored.deploymentRevision } : patch;
}

/** A local apply's failure as a preview: the engine's rejection, or why there is no preview. */
function failedPreview(thrown: unknown): Preview {
  if (thrown instanceof HostFailure && thrown.reason.error === "rejected") {
    return { outcome: "rejected", rejection: thrown.reason.rejection };
  }
  return { outcome: "unavailable", message: thrown instanceof Error ? thrown.message : String(thrown) };
}

/** What a preview is made from: the derive worker, the author, and what is previewed. */
interface PreviewArgs {
  deriver: Deriver;
  user: string;
  authored: Authored;
  mutations: Mutation[];
}

/** The preview of `mutations` against `authored`, applied locally by the derive worker. */
export async function previewOf({ deriver, user, authored, mutations }: PreviewArgs): Promise<Preview> {
  const patch = authoredPatchOf(authored, mutations);
  const actor = { user };
  const at = new Date().toISOString();
  try {
    if ("journey" in authored.target) {
      const applied = await deriver.apply(authored.target.journey, { patch, at, actor });
      return { outcome: "accepted", consequences: applied.consequences };
    }
    const request = { patch, at, actor, today: authored.today, deployment: authored.deployment };
    await deriver.applyRoute(authored.route === undefined ? request : { ...request, route: authored.route });
    return { outcome: "accepted", consequences: undefined };
  } catch (thrown) {
    return failedPreview(thrown);
  }
}

/** The preview of `mutations` against `authored`, applied locally by the derive worker. */
export function usePreviewer(authored: Authored): (mutations: Mutation[]) => Promise<Preview> {
  const { deriver } = useSession();
  const { viewer } = useViewer();
  return (mutations) => previewOf({ deriver, user: viewer?.user ?? LOCAL_USER, authored, mutations });
}

/** What a publish preview is made from. */
interface PublishArgs {
  deriver: Deriver;
  host: Host;
  user: string;
  authored: Authored;
}

/**
 * The preview of publishing a route's draft: what it would be refused for (A15). Unlike an edit, a
 * publish reads the versions the route already has, so they are read first; `at` (a keyed read's)
 * is asked once they are in, so a preview nobody waits for any longer posts nothing.
 */
async function publishPreviewOf({ deriver, host, user, authored }: PublishArgs, at?: Pick<ReadAt<unknown>, "signal" | "compatible">): Promise<Preview> {
  const route = authored.route;
  if (route === undefined || !("route" in authored.target)) {
    return { outcome: "unavailable", message: "Only a route's draft is published." };
  }
  try {
    const versions = await Promise.all((route.versions ?? []).map((version) => host.routeVersion(route.header.id, version)));
    if (at !== undefined) {
      beforeWork(at);
    }
    const patch = authoredPatchOf(authored, [{ op: "publish_draft" }]);
    await deriver.applyRoute({ patch, at: new Date().toISOString(), actor: { user }, today: authored.today, deployment: authored.deployment, route, versions });
    return { outcome: "accepted", consequences: undefined };
  } catch (thrown) {
    if (thrown instanceof DOMException || thrown instanceof Skewed) {
      throw thrown;
    }
    return failedPreview(thrown);
  }
}

const PUBLISH_PREVIEW: ReadSource<PublishArgs, Preview> = { policy: DERIVE, read: (args, at) => publishPreviewOf(args, at) };

/** What publishing `authored`'s draft would be refused for (A15), read again whenever the draft or the deployment moves. */
export function usePublishPreviewRead(authored: Authored): Preview | undefined {
  const { deriver, host } = useSession();
  const { viewer } = useViewer();
  const user = viewer?.user ?? LOCAL_USER;
  const key: ReadKey<PublishArgs> = { identity: [JSON.stringify(authored.target)], freshness: [authored.graph, authored.deploymentRevision, authored.today, user], args: { deriver, host, user, authored } };
  const { view } = useKeyedRead(PUBLISH_PREVIEW, key);
  return view.status === "current" ? view.value : undefined;
}

/** Authoring's write path for `authored`'s graph. */
export function useAuthorWrite(authored: Authored): AuthorWrite {
  const session = useSession();
  const skew = useSkew();
  const preview = usePreviewer(authored);
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState<AuthorWrite["failed"]>();
  const [unanswered, setUnanswered] = useState<AuthorWrite["unanswered"]>();
  const receipt = useReceipt();
  const key = `author:${useId()}`;
  const dismiss = () => {
    setFailed(undefined);
  };
  useProblem(key, failed === undefined ? undefined : unlandedOf(failed.rejection), { discard: dismiss });
  useEffect(() => () => { session.edits.send({ type: "resolve", key }); }, [session, key]);
  const run = async (mutations: Mutation[], places: Place[] = [], base = authored.revision): Promise<boolean> => {
    const sent = { mutations, places };
    setPending(true);
    receipt.clear();
    const previewed = await preview(mutations);
    if (previewed.outcome === "rejected") {
      setPending(false);
      setFailed({ sent, rejection: previewed.rejection });
      return false;
    }
    const patch = authoredPatchOf(authored, mutations, base);
    const result = await session.write({
      target: patch.target,
      baseRevision: patch.base_revision,
      mutations,
      ...(patch.deployment_revision === undefined || patch.deployment_revision === null ? {} : { deploymentRevision: patch.deployment_revision }),
    });
    setPending(false);
    setFailed(result.outcome === "rejected" ? { sent, rejection: result.rejection } : undefined);
    setUnanswered(result.outcome === "failed" || result.outcome === "stopped" || result.outcome === "gone" ? result.outcome : undefined);
    if (result.outcome === "landed") {
      receipt.show(result.warning);
    }
    return result.outcome === "landed";
  };
  return { run, pending, disabled: pending || skew, failed, unanswered, dismiss, receipt: receipt.receipt };
}

/** How long typing settles before a form's preview runs. */
export const PREVIEW_SETTLE_MS = 250;

/** A preview of what a form would send, read once its settle has passed (or at once). */
const previewSource = (policy: ReadPolicy): ReadSource<PreviewArgs, Preview> => ({
  policy,
  read: (args, at) => {
    beforeWork(at);
    return previewOf(args);
  },
});

export const LIVE_PREVIEW = previewSource(previewPolicy(PREVIEW_SETTLE_MS));
const AT_ONCE_PREVIEW = previewSource(DERIVE);

/** The key of a preview: the target is the identity; the revisions it is read against, the day, the viewer and the mutations are what it is read from. */
export function previewKey(deriver: Deriver, user: string, authored: Authored, mutations: Mutation[]): ReadKey<PreviewArgs> {
  const { target, revision, deploymentRevision, today, route } = authored;
  return { identity: [JSON.stringify(target)], freshness: [revision, deploymentRevision, today, route?.revision, user, JSON.stringify(mutations)], args: { deriver, user, authored, mutations } };
}

function usePreviewRead(source: ReadSource<PreviewArgs, Preview>, authored: Authored, mutations: Mutation[] | undefined): Preview | undefined {
  const { deriver } = useSession();
  const { viewer } = useViewer();
  const asked = mutations === undefined || mutations.length === 0 ? undefined : mutations;
  const { view } = useKeyedRead(source, asked === undefined ? undefined : previewKey(deriver, viewer?.user ?? LOCAL_USER, authored, asked));
  return view.status === "current" ? view.value : undefined;
}

/** The preview of what a form would send now, once typing settles; none while there is nothing to send. */
export function useLivePreview(authored: Authored, mutations: Mutation[] | undefined): Preview | undefined {
  return usePreviewRead(LIVE_PREVIEW, authored, mutations);
}

/** The preview of a patch a dialog proposes, at once: it is not being typed. */
export function useDialogPreview(authored: Authored, mutations: Mutation[]): Preview | undefined {
  return usePreviewRead(AT_ONCE_PREVIEW, authored, mutations);
}
