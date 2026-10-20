// H3: when Cairn asks what to call the caller. Pure, so the rule is tested without a screen.
import type { Caller, Deployment } from "../data/host.ts";
import type { Identity } from "../people/model.ts";

/** H3: the caller's identity that has no name yet (their first by key), or none when it has one or the caller has no identity in the deployment. */
export function asksName(caller: Caller | undefined, deployment: Deployment | undefined): Identity | undefined {
  if (caller === undefined || deployment === undefined) {
    return undefined;
  }
  const first = [...caller.identities].sort()[0];
  const identity = (deployment.identities ?? []).find((candidate) => candidate.key === first);
  return identity !== undefined && (identity.name === undefined || identity.name === null) ? identity : undefined;
}
