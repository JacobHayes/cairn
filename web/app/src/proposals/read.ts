// A proposal under review, kept current (H6, I6): the host's preview of it as held (the
// proposal at its editing revision, its unresolved items and violations, the graph and
// frontier after, and whether its destination moved since it was drafted), read again when
// the proposal moves or its destination does. Its address, and the destination's words.
import type { RevisionOf } from "@cairn/client";

import type { LiveSpec } from "../data/live.ts";
import type { Domain, ProposalReview } from "../data/proposals.ts";
import type { Session } from "../data/session.ts";

/** C14: a proposal's review screen. */
export function proposalPath(id: string): string {
  return `/proposals/${id}`;
}

/** The watch name of a destination (`journey:<id>`, `route:<id>`, `deployment`). */
export function watchOf(destination: Domain): string {
  if (destination === "deployment") {
    return "deployment";
  }
  return "journey" in destination ? `journey:${destination.journey}` : `route:${destination.route}`;
}

function sameDomain(left: Domain, right: Domain): boolean {
  if (left === "deployment" || right === "deployment") {
    return left === right;
  }
  if ("journey" in left) {
    return "journey" in right && left.journey === right.journey;
  }
  return "route" in right && left.route === right.route;
}

/** The destination's revision now, as the review read it: the current one when it moved. */
export function destinationRevision(review: ProposalReview): number {
  return review.stale?.conflict.current ?? review.proposal.draft.destination_revision;
}

/**
 * I6: proposal `id`'s review, kept current: its own ticks, and its destination's, which a
 * first read names. A destination that moves past the revision the proposal was drafted
 * against makes the next read stale, with what the intervening events touched.
 */
export function proposalReview(id: string) {
  return (session: Session): LiveSpec<ProposalReview> => {
    let destination: Domain | undefined;
    const isProposal = (of: RevisionOf) => "proposal" in of && of.proposal === id;
    return {
      watching: [`proposal:${id}`, "journeys", "routes", "deployment"],
      about: (of) => isProposal(of) || ("domain" in of && destination !== undefined && sameDomain(of.domain, destination)),
      fetch: async () => {
        const review = await session.host.proposals.preview(id);
        destination = review.proposal.destination;
        return review;
      },
      holds: (review) => [
        { of: { proposal: id }, revision: review.proposal.revision },
        { of: { domain: review.proposal.destination }, revision: destinationRevision(review) },
      ],
    };
  };
}
