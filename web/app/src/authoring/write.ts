// Authoring's write path: every edit is previewed with the wasm engine before it is sent
// (ARCHITECTURE, Web UI: Previews), so a rejection shows at the field without a round trip,
// then sent through the edit manager (H5's safe retry, D7's notice, nothing sent under version
// skew). The preview is the write's first phase: a refusal is a rejected write like the
// server's, kept in the manager and counted on the chip, and nothing is sent. A journey's patch
// is applied to the document the derive worker holds; a route's to the route as read. Committed
// state always comes from the host. The preview of what a form would send also runs as the
// author types, so its fields show what the engine would say before Save.
import type { Schema } from "@cairn/client";
import { HostFailure } from "@cairn/wasm";

import type { Command } from "../data/edit-command.ts";
import { unansweredOf } from "../data/edit-manager.ts";
import type { Deriver, Host } from "../data/host.ts";
import { Skewed } from "../data/host.ts";
import { beforeWork, DERIVE, previewPolicy, type ReadAt, type ReadKey, type ReadPolicy, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession, useCaller, useWrite } from "../data/react.ts";
import type { WriteView } from "../data/write-machine.ts";
import type { ReceiptState } from "../ui/receipt.ts";
import { newPatchId, type Rejection } from "../data/writes.ts";
import type { Mutation } from "./graph.ts";
import { domainOf, type Authored } from "./target.ts";
import { placesOf, type Place } from "./violations.ts";

/** What the engine says of a patch before it is sent. */
export type Preview =
  | { outcome: "accepted"; consequences: Schema<"Consequences"> | undefined }
  | { outcome: "rejected"; rejection: Rejection }
  | { outcome: "unavailable"; message: string };

/** A patch and the form places its mutations came from, by position (A15). */
export interface Sent {
  mutations: Mutation[];
  places: (Place | undefined)[];
}

export interface AuthorWrite {
  /**
   * Previews `mutations` on the graph as held, then sends them drafted against `base` (the
   * revision their author started from, H5; the one held by default) unless the preview
   * rejects them; true when they landed.
   */
  run: (mutations: Mutation[], base?: number) => Promise<boolean>;
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
  /** What the write is doing when it is not rejected: still resending, not sent, or its subject gone. */
  status: WriteView | undefined;
  dismiss: () => void;
  /** What the last landed write leaves under the form, for a few seconds. */
  receipt: ReceiptState | undefined;
}

/** Who a preview is made as when the caller is not known yet. */
export const LOCAL_USER = "u_local";

/**
 * The patch `mutations` make of `authored`'s graph, drafted against `base` and named `id`. A
 * journey's patch names the deployment revision it saw (E6): the service holds the patch that
 * leaves an identity reference to it, so the page does not decide which writes do.
 */
export function authoredPatchOf(authored: Authored, mutations: Mutation[], base = authored.revision, id = newPatchId()): Schema<"Patch"> {
  const patch: Schema<"Patch"> = { id, target: authored.target, base_revision: base, mutations };
  return "journey" in authored.target ? { ...patch, deployment_revision: authored.deploymentRevision } : patch;
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
  const { caller } = useCaller();
  const user = caller?.user ?? LOCAL_USER;
  const key: ReadKey<PublishArgs> = { of: [JSON.stringify(authored.target)], freshness: [authored.graph, authored.deploymentRevision, authored.today, user], args: { deriver, host, user, authored } };
  const { view } = useKeyedRead(PUBLISH_PREVIEW, key);
  return view.status === "current" ? view.value : undefined;
}

/** The edit an authoring control's writes are, as the `scope` it names (the control and its node): stable, so a remounted control finds its write. */
export function authorEdit(authored: Authored, scope: string): string {
  return `author:${domainOf(authored)}:${scope}`;
}

/** The patch authoring sends for `mutations`, drafted against `base` under the patch id `id`. */
export function authorCommand(authored: Authored, mutations: Mutation[], base: number, id: string = newPatchId()): Command {
  return { kind: "patch", patch: authoredPatchOf(authored, mutations, base, id) };
}

/** The write's first phase: what the engine says of `mutations`, as the rejection that stops it, or none. */
async function checkOf(args: PreviewArgs): Promise<Rejection | undefined> {
  const previewed = await previewOf(args);
  return previewed.outcome === "rejected" ? previewed.rejection : undefined;
}

/**
 * Authoring's write path for `authored`'s graph, as the edit `scope` names within it. The
 * write is the manager's: the control reads its rejection (the engine's preview or the host's)
 * and its status from the manager, so one view answers for both.
 */
export function useAuthorWrite(authored: Authored, scope: string, label?: string): AuthorWrite {
  const { deriver } = useSession();
  const { caller } = useCaller();
  const write = useWrite(authorEdit(authored, scope), label);
  const sent = write.write?.command.kind === "patch" ? write.write.command.patch.mutations : undefined;
  return {
    run: async (mutations, base = authored.revision) => {
      const check = () => checkOf({ deriver, user: caller?.user ?? LOCAL_USER, authored, mutations });
      return (await write.submit(authorCommand(authored, mutations, base), check)) !== undefined;
    },
    pending: write.busy,
    disabled: write.disabled,
    failed: write.rejection === undefined || sent === undefined ? undefined : { sent: { mutations: sent, places: placesOf(sent) }, rejection: write.rejection },
    unanswered: unansweredOf(write.write),
    status: write.status,
    dismiss: write.dismiss,
    receipt: write.receipt,
  };
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

/** The key of a preview: the target is what it is of; the revisions it is read against, the day, the viewer and the mutations are what it is read from. */
export function previewKey(deriver: Deriver, user: string, authored: Authored, mutations: Mutation[]): ReadKey<PreviewArgs> {
  const { target, revision, deploymentRevision, today, route } = authored;
  return { of: [JSON.stringify(target)], freshness: [revision, deploymentRevision, today, route?.revision, user, JSON.stringify(mutations)], args: { deriver, user, authored, mutations } };
}

function usePreviewRead(source: ReadSource<PreviewArgs, Preview>, authored: Authored, mutations: Mutation[] | undefined): Preview | undefined {
  const { deriver } = useSession();
  const { caller } = useCaller();
  const asked = mutations === undefined || mutations.length === 0 ? undefined : mutations;
  const { view } = useKeyedRead(source, asked === undefined ? undefined : previewKey(deriver, caller?.user ?? LOCAL_USER, authored, asked));
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
