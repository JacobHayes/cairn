// A proposal under review, kept current (H6, I6): the host's preview of it as held (the
// proposal at its editing revision, its unresolved items and violations, the graph and
// frontier after, and whether its destination moved since it was drafted), read again when
// the proposal moves or its destination does. Its address, and the destination's words.
import { sameDomain, watchName, type RevisionOf } from "@cairn/client";

import type { LiveSpec } from "../data/live.ts";
import type { Cursor, Domain, Listing, ProposalList, ProposalReview, ProposalSummary } from "../data/proposals.ts";
import type { Session } from "../data/session.ts";

/** The watch name of a destination (`journey:<id>`, `route:<id>`, `deployment`). */
const watchOf = (destination: Domain): string => watchName({ domain: destination });

/** The destination's revision now, as the review read it: the current one when it moved. */
export function destinationRevision(review: ProposalReview): number {
  return review.stale?.conflict.current ?? review.proposal.draft.destination_revision;
}

/** The live-read key of a destination, in the words the watch names use. */
function keyOf(destination: Domain): string {
  return watchOf(destination);
}

/** The key of proposal `id`'s review under `destination`. */
export function reviewKey(destination: Domain, id: string): string {
  return `proposal:${keyOf(destination)}:${id}`;
}

/**
 * I6: proposal `id`'s review under `destination`, kept current: its own ticks and its
 * destination's, watched from the first read. A destination that moves past the revision the
 * proposal was drafted against makes the next read stale, with what the intervening events
 * touched.
 */
export function proposalReview(destination: Domain, id: string) {
  return (session: Session): LiveSpec<ProposalReview> => {
    const isProposal = (of: RevisionOf) => "proposal" in of && of.proposal === id;
    return {
      watching: [`proposal:${id}`, watchOf(destination)],
      about: (of) => isProposal(of) || ("domain" in of && sameDomain(of.domain, destination)),
      fetch: () => session.host.proposals.preview(destination, id),
      holds: (review) => [
        { of: { proposal: id }, revision: review.proposal.revision },
        { of: { domain: destination }, revision: destinationRevision(review) },
      ],
    };
  };
}

/** What a list of open proposals holds: every page's rows, how many there are, and the destination as it is now. */
export interface Proposals {
  count: number;
  destination: ProposalList["destination"];
  items: ProposalSummary[];
}

async function everyPage(read: (after: Cursor | undefined) => Promise<ProposalList>): Promise<Proposals> {
  const first = await read(undefined);
  const items = [...first.items];
  let next = first.next;
  while (next != null) {
    const page = await read(next);
    items.push(...page.items);
    next = page.next;
  }
  return { count: first.count, destination: first.destination, items };
}

/** What a list holds, for a tick to compare: each row's editing revision, and the destination's. */
function listHolds(found: Proposals, destination?: Domain): { of: RevisionOf; revision: number }[] {
  const rows = found.items.map((item) => ({ of: { proposal: item.id }, revision: item.revision }));
  return destination === undefined || found.destination == null ? rows : [...rows, { of: { domain: destination }, revision: found.destination.revision }];
}

/** C14: the open proposals under `destination`, kept current: a proposal drafted, edited, applied or discarded, or the destination moving. */
export function proposalsUnder(destination: Domain) {
  return (session: Session): LiveSpec<Proposals> => ({
    watching: ["proposals", watchOf(destination)],
    about: (of) => "proposal" in of || ("domain" in of && sameDomain(of.domain, destination)),
    fetch: () => everyPage((after) => session.host.proposals.list(destination, after)),
    holds: (found) => listHolds(found, destination),
  });
}

/** C14, C16: the open proposals no one destination holds (those waiting on the viewer, those that would create a journey or a route), kept current. */
export function proposalsListed(scope: Listing) {
  return (session: Session): LiveSpec<Proposals> => ({
    watching: scope === "waiting" ? ["proposals", "journeys", "routes", "deployment"] : ["proposals", scope === "new_journeys" ? "journeys" : "routes"],
    about: (of) => "proposal" in of || ("domain" in of && of.domain !== "deployment" && (scope === "new_routes" ? "route" in of.domain : "journey" in of.domain)) || (scope === "waiting" && "domain" in of && (of.domain === "deployment" || "route" in of.domain)),
    fetch: () => everyPage((after) => session.host.proposals.listing(scope, after)),
    holds: (found) => listHolds(found),
  });
}

/** The live-read key of a list of proposals. */
export const listKey = (scope: Domain | Listing): string => `proposals:${typeof scope === "string" ? scope : keyOf(scope)}`;
