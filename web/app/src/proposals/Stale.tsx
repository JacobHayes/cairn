// I6: a proposal whose destination moved since it was drafted. It shows what moved: for a
// journey, the patches committed since (its last ones, one per revision, from its history);
// for anything else, each record the intervening events touched. Refreshing drafts it again on the
// destination as it stands, the reviewer's choices carried over, at a new revision, so it is
// reviewed again before it can be applied.
import type { Schema } from "@cairn/client";
import { useEffect, useState } from "react";

import { allPages } from "../data/live.ts";
import type { ProposalReview } from "../data/proposals.ts";
import { useSession } from "../data/react.ts";
import { describeRecord } from "../screens/RejectionView.tsx";
import { Button } from "../ui/kit.tsx";
import { intervening } from "./model.ts";

type PatchEvents = Schema<"PatchEvents">;

/** I6: journey `journey`'s patches between `expected` and `current`, read from its history. */
function useIntervening(journey: string | undefined, expected: number, current: number): PatchEvents[] | { failed: string } | undefined {
  const { host } = useSession();
  const [read, setRead] = useState<PatchEvents[] | { failed: string } | undefined>();
  useEffect(() => {
    if (journey === undefined) {
      return;
    }
    let live = true;
    allPages<PatchEvents, number>(async (after) => {
      const page = await host.history(journey, undefined, after);
      return { items: page.patches, next: page.next ?? null };
    }).then(
      (patches) => {
        if (live) {
          setRead(intervening(joined(patches), expected, current));
        }
      },
      (thrown: unknown) => {
        if (live) {
          setRead({ failed: thrown instanceof Error ? thrown.message : String(thrown) });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [host, journey, expected, current]);
  return read;
}

/** Pages joined back into whole patches: one split across two pages is one patch. */
function joined(patches: PatchEvents[]): PatchEvents[] {
  const whole: PatchEvents[] = [];
  for (const patch of patches) {
    const last = whole.at(-1);
    if (last?.patch_id === patch.patch_id) {
      whole[whole.length - 1] = { ...last, events: [...last.events, ...patch.events] };
    } else {
      whole.push(patch);
    }
  }
  return whole;
}

function PatchLine({ patch }: { patch: PatchEvents }) {
  const [first] = patch.events;
  if (first === undefined) {
    return null;
  }
  const kinds = [...new Set(patch.events.map((event) => event.event_type.replaceAll("_", " ")))];
  return (
    <li data-testid="intervening-patch">
      <span className="muted">
        {first.at} by {first.actor.user}
      </span>{" "}
      {kinds.join(", ")}
    </li>
  );
}

export interface StaleProps {
  review: ProposalReview;
  disabled: boolean;
  onRefresh: () => void;
}

/** I6: what moved since the proposal was drafted, and its refresh. */
export function StalePanel({ review, disabled, onRefresh }: StaleProps) {
  const stale = review.stale;
  const destination = review.proposal.destination;
  const journey = destination !== "deployment" && "journey" in destination ? destination.journey : undefined;
  const patches = useIntervening(stale == null ? undefined : journey, stale?.conflict.expected ?? 0, stale?.conflict.current ?? 0);
  if (stale == null) {
    return null;
  }
  return (
    <section className="callout stack" role="alert" data-testid="stale" data-expected={stale.conflict.expected} data-current={stale.conflict.current}>
      <strong>
        This proposal was drafted at revision {stale.conflict.expected}; its {journey === undefined ? "destination" : "journey"} is at {stale.conflict.current} now. Refresh it, then review it again before applying.
      </strong>
      {journey === undefined ? (
        <ul className="detail-list" data-testid="intervening">
          {[...new Set(stale.intervening.map(describeRecord))].map((record) => (
            <li key={record} data-testid="intervening-record">
              {record}
            </li>
          ))}
        </ul>
      ) : patches === undefined ? (
        <span className="muted">Reading what changed...</span>
      ) : "failed" in patches ? (
        <span className="author-problem">What changed could not be read: {patches.failed}</span>
      ) : (
        <ol className="detail-list" data-testid="intervening">
          {patches.map((patch) => (
            <PatchLine key={patch.patch_id} patch={patch} />
          ))}
        </ol>
      )}
      <span className="row">
        <Button primary disabled={disabled} onClick={onRefresh} data-testid="refresh">
          Refresh against the current {journey === undefined ? "destination" : "journey"}
        </Button>
      </span>
    </section>
  );
}
