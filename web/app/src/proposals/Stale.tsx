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
import { useJourney, useKeyedRead, useSession, useUserWords } from "../data/react.ts";
import { listed, recount, wordsOf } from "../detail/happened.ts";
import { useRecordWords } from "../screens/RejectionView.tsx";
import { Button } from "../ui/kit.tsx";
import { intervening } from "./model.ts";

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
    return intervening(joined(patches), expected, current);
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

/** The patches that came between, each as who did what, in order. */
function InterveningPatches({ journey, patches }: { journey: string; patches: PatchEvents[] }) {
  const view = useJourney(journey);
  const name = useUserWords();
  const told = recount(
    patches.flatMap((patch) => patch.events),
    wordsOf(view.status === "ready" ? view : undefined, "", name),
  );
  let at = 0;
  return (
    <ol className="detail-list" data-testid="intervening">
      {patches.map((patch) => {
        const mine = told.slice(at, at + patch.events.length);
        at += patch.events.length;
        const [first] = patch.events;
        const [lead] = mine;
        return first === undefined || lead === undefined ? null : (
          <li key={patch.patch_id} data-testid="intervening-patch">
            <span className="muted small">{first.at.slice(0, 10)}</span> {name(lead.actor)} {listed([...new Set(mine.map((each) => each.what))])}
          </li>
        );
      })}
    </ol>
  );
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
      <strong>
        This proposal was drafted before its {journey === undefined ? "destination" : "journey"} last changed. Refresh it, then review it again before applying.
      </strong>
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
        <span className="author-problem">What changed could not be read: {patches.failed}</span>
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
