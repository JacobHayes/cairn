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

import { unlandedOf, useProblem, useSession, useSkew, useViewer } from "../data/react.ts";
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
  dismiss: () => void;
  /** What the last landed write leaves under the form, for a few seconds. */
  receipt: ReceiptState | undefined;
}

const LOCAL_USER = "u_local";

function patchOf(authored: Authored, mutations: Mutation[], base = authored.revision): Schema<"Patch"> {
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

/** The preview of `mutations` against `authored`, applied locally by the derive worker. */
export function usePreviewer(authored: Authored): (mutations: Mutation[]) => Promise<Preview> {
  const { deriver } = useSession();
  const { viewer } = useViewer();
  return async (mutations) => {
    const patch = patchOf(authored, mutations);
    const actor = { user: viewer?.user ?? LOCAL_USER };
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
  };
}

/** Authoring's write path for `authored`'s graph. */
export function useAuthorWrite(authored: Authored): AuthorWrite {
  const session = useSession();
  const skew = useSkew();
  const preview = usePreviewer(authored);
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState<AuthorWrite["failed"]>();
  const receipt = useReceipt();
  const key = `author:${useId()}`;
  const dismiss = () => {
    setFailed(undefined);
  };
  useProblem(key, failed === undefined ? undefined : unlandedOf(failed.rejection), { discard: dismiss });
  useEffect(() => () => { session.sync.resolve(key); }, [session, key]);
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
    const patch = patchOf(authored, mutations, base);
    const result = await session.write({
      target: patch.target,
      baseRevision: patch.base_revision,
      mutations,
      ...(patch.deployment_revision === undefined || patch.deployment_revision === null ? {} : { deploymentRevision: patch.deployment_revision }),
    });
    setPending(false);
    setFailed(result.outcome === "rejected" ? { sent, rejection: result.rejection } : undefined);
    if (result.outcome === "landed") {
      receipt.show(result.warning);
    }
    return result.outcome === "landed";
  };
  return { run, pending, disabled: pending || skew !== undefined, failed, dismiss, receipt: receipt.receipt };
}

/** How long typing settles before a form's preview runs. */
export const PREVIEW_SETTLE_MS = 250;

/** The preview of what a form would send now, once typing settles; none while there is nothing to send. */
export function useLivePreview(authored: Authored, mutations: Mutation[] | undefined): Preview | undefined {
  const previewer = usePreviewer(authored);
  const asked = mutations === undefined || mutations.length === 0 ? "" : JSON.stringify([authored.revision, mutations]);
  const [answered, setAnswered] = useState<{ for: string; preview: Preview } | undefined>(undefined);
  useEffect(() => {
    if (asked === "" || mutations === undefined) {
      return;
    }
    let live = true;
    const timer = setTimeout(() => {
      void previewer(mutations).then((preview) => {
        if (live) {
          setAnswered({ for: asked, preview });
        }
      });
    }, PREVIEW_SETTLE_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // `asked` names the mutations and the revision they are drafted against, so it alone is the key.
  }, [asked]);
  return answered?.for === asked ? answered.preview : undefined;
}
