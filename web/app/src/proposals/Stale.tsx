// I6: a proposal whose destination moved since it was drafted. It shows what moved: for a
// journey, the patches committed since (its last ones, one per revision, from its history);
// for anything else, each record the intervening events touched. Refreshing drafts it again on the
// destination as it stands, the reviewer's choices carried over, at a new revision, so it is
// reviewed again before it can be applied.
import type { Schema } from "@cairn/client";
import type { Host } from "../data/host.ts";
import { HOST_READ, type ReadSource } from "../data/keyed-read.ts";
import { allPages } from "../data/live.ts";
import type { ProposalReview } from "../data/proposals.ts";
import { useJourney, useKeyedRead, useSession, useToday } from "../data/react.ts";
import { joinPatches, PatchList } from "../detail/History.tsx";
import { useRecordWords } from "../screens/RejectionView.tsx";
import { Button } from "../ui/kit.tsx";
import { intervening } from "./model.ts";
import { Failed } from "../ui/Failed.tsx";

type PatchEvents = Schema<"PatchEvents">;

/** What the intervening patches are read from. */
interface InterveningArgs {
  host: Host;
  journey: string;
  expected: number;
  current: number;
}

const INTERVENING: ReadSource<InterveningArgs, PatchEvents[]> = {
  policy: HOST_READ,
  read: async ({ host, journey, expected, current }) => {
    const patches = await allPages<PatchEvents, number>(async (after) => {
      const page = await host.history(journey, undefined, after);
      return { items: page.patches, next: page.next ?? null };
    });
    return intervening(joinPatches(patches), expected, current);
  },
};

/** I6: journey `journey`'s patches between `expected` and `current`, read from its history. */
function useIntervening(journey: string | undefined, expected: number, current: number): PatchEvents[] | { failed: string } | undefined {
  const { host } = useSession();
  const { view } = useKeyedRead(INTERVENING, journey === undefined ? undefined : { identity: [journey], freshness: [expected, current], args: { host, journey, expected, current } });
  if (view.status === "current") {
    return view.value;
  }
  return view.status === "failed" || view.status === "behind" ? { failed: view.error.message } : undefined;
}

/** The patches that came between, each as who did what, in order. */
function InterveningPatches({ journey, patches }: { journey: string; patches: PatchEvents[] }) {
  const view = useJourney(journey);
  const sessionToday = useToday();
  const ready = view.status === "ready" ? view : undefined;
  return <PatchList scope={{ view: ready, node: "", today: ready?.key.today ?? sessionToday }} patches={patches} testId="intervening" />;
}

export interface StaleProps {
  review: ProposalReview;
  disabled: boolean;
  onRefresh: () => void;
}

/** I6: what moved since the proposal was drafted, and its refresh. */
export function StalePanel({ review, disabled, onRefresh }: StaleProps) {
  const words = useRecordWords();
  const stale = review.stale;
  const destination = review.proposal.destination;
  const journey = destination !== "deployment" && "journey" in destination ? destination.journey : undefined;
  const patches = useIntervening(stale == null ? undefined : journey, stale?.conflict.expected ?? 0, stale?.conflict.current ?? 0);
  if (stale == null) {
    return null;
  }
  return (
    <section className="callout stack" role="alert" data-testid="stale" data-expected={stale.conflict.expected} data-current={stale.conflict.current}>
      <strong>This proposal is out of date.</strong>
      <span>It was drafted before its {journey === undefined ? "destination" : "journey"} last changed. Refresh it, then review it again before applying.</span>
      {journey === undefined ? (
        <ul className="detail-list" data-testid="intervening">
          {[...new Set(stale.intervening.map(words))].map((record) => (
            <li key={record} data-testid="intervening-record">
              {record}
            </li>
          ))}
        </ul>
      ) : patches === undefined ? (
        <span className="muted small">Reading what changed...</span>
      ) : "failed" in patches ? (
        <span className="author-problem"><Failed what="What changed" error={patches.failed} /></span>
      ) : (
        <InterveningPatches journey={journey} patches={patches} />
      )}
      <span className="row">
        <Button primary disabled={disabled} onClick={onRefresh} data-testid="refresh">
          Refresh against the current {journey === undefined ? "destination" : "journey"}
        </Button>
      </span>
    </section>
  );
}
