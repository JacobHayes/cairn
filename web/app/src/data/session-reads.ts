// The two reads the session itself keeps for the tab's whole life (ARCHITECTURE, Web UI:
// reads): the deployment context (E6: entities, emails, aliases; owners and entities feed
// rank), watched and read again on a deployment tick newer than the revision held (H6), and the
// caller (H3), which is the user, the identities they sign in with, and the entities their
// verified emails name. Which entities hold which emails is the deployment's, so the root reads
// the caller again for each newer deployment it holds.
import type { RevisionOf } from "@cairn/client";

import type { Deployment, Host, Viewer } from "./host.ts";
import { HOST_READ, heldValue, type ReadKey, type ReadSnapshot, type ReadSource } from "./keyed-read.ts";

const DEPLOYMENT: RevisionOf = { domain: "deployment" };

/** What the deployment read is keyed by: one identity for the tab's life, read from nothing else. */
export const DEPLOYMENT_KEY: ReadKey<undefined> = { identity: ["deployment"], freshness: [], args: undefined };

/** The deployment context, never older than the one held. */
export function deploymentSource(host: Host): ReadSource<undefined, Deployment> {
  return {
    policy: HOST_READ,
    async read(_, at) {
      const deployment = await host.deployment();
      return at.held !== undefined && deployment.revision < at.held.revision ? at.held : deployment;
    },
    ticks: () => ({
      watching: ["deployment"],
      about: (of) => "domain" in of && of.domain === "deployment",
      holds: (value) => [{ of: DEPLOYMENT, revision: value.revision }],
    }),
  };
}

/** The caller. */
export function viewerSource(host: Host): ReadSource<undefined, Viewer> {
  return { policy: HOST_READ, read: () => host.viewer() };
}

/**
 * The caller's key once the deployment's first read has settled, held or failed, so the caller is
 * read once whatever the deployment answered, and again for each newer deployment held.
 */
export function viewerKey(deployment: ReadSnapshot): ReadKey<undefined> | undefined {
  if (deployment.context.attempted === undefined || deployment.hasTag("busy")) {
    return undefined;
  }
  return { identity: ["viewer"], freshness: [heldValue<Deployment>(deployment)?.revision], args: undefined };
}
