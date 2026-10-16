// ARCHITECTURE, Web UI: previews. The graph an insertion would leave, from the tab's own
// engine over the graph as held (committing nothing), so the stepper draws the incoming root
// where review will; or why the engine would refuse it. Debounced, so a name being typed
// is not asked for on every keystroke.
import { HostFailure } from "@cairn/wasm";
import type { Graph, Mutation } from "../authoring/graph.ts";
import type { Authored } from "../authoring/target.ts";
import { Skewed, type Deriver } from "../data/host.ts";
import { beforeWork, previewPolicy, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession, useViewer } from "../data/react.ts";
import { newPatchId, reasonOf } from "../data/writes.ts";
import type { SegmentVersion } from "./read.ts";

/** How long the choices must hold still before they are previewed (ms). */
const SETTLE_MS = 200;

export type Candidate = { status: "pending" } | { status: "refused"; reasons: string[] } | { status: "placed"; before: Graph; after: Graph };

/** What stops a preview: the engine's violations, each its own sentence, or the failure's words. */
function reasonsOf(thrown: unknown): string[] {
  if (thrown instanceof HostFailure && thrown.reason.error === "rejected") {
    const rejection = thrown.reason.rejection;
    return rejection.rejection === "invalid" && rejection.violations.length > 0 ? rejection.violations.map((violation) => violation.message) : [reasonOf(rejection)];
  }
  return [thrown instanceof Error ? thrown.message : String(thrown)];
}

/** What a candidate is made from. */
interface CandidateArgs {
  deriver: Deriver;
  user: string;
  authored: Authored;
  mutation: Mutation;
  segment: SegmentVersion;
}

/** The engine's answer for the insertion: the graph it leaves, or why it refuses (a value, not a failure). */
const CANDIDATE: ReadSource<CandidateArgs, Candidate> = {
  policy: previewPolicy(SETTLE_MS),
  read: async ({ deriver, user, authored, mutation, segment }, at) => {
    beforeWork(at);
    const patch = { id: newPatchId(), target: authored.target, base_revision: authored.revision, mutations: [mutation] };
    const actor = { user };
    const now = new Date().toISOString();
    try {
      if ("journey" in authored.target) {
        const applied = await deriver.apply(authored.target.journey, { patch, at: now, actor, versions: [segment.version], segments: [segment.segment] });
        return { status: "placed", before: authored.graph, after: applied.document.journey.graph };
      }
      const route = await deriver.applyRoute({ patch, at: now, actor, today: authored.today, deployment: authored.deployment, versions: [segment.version], segments: [segment.segment], ...(authored.route === undefined ? {} : { route: authored.route }) });
      return { status: "placed", before: authored.graph, after: route.draft?.graph ?? authored.graph };
    } catch (thrown) {
      if (thrown instanceof DOMException || thrown instanceof Skewed) {
        throw thrown;
      }
      return { status: "refused", reasons: reasonsOf(thrown) };
    }
  },
};

/** The graph `authored` would hold after `mutation` inserts `segment`, or why not; pending while it settles. */
export function useCandidate(authored: Authored, mutation: Mutation | undefined, segment: SegmentVersion | undefined): Candidate {
  const { deriver } = useSession();
  const { viewer } = useViewer();
  const user = viewer?.user ?? "u_local";
  const { view } = useKeyedRead(
    CANDIDATE,
    mutation === undefined || segment === undefined
      ? undefined
      : {
          identity: [JSON.stringify(authored.target)],
          // The segment's route revision and version are what it inserts; the deployment and the day are what the engine reads besides the graph.
          freshness: [authored.revision, authored.deploymentRevision, authored.today, authored.route?.revision, JSON.stringify(mutation), segment.version.version, segment.segment.revision, user],
          args: { deriver, user, authored, mutation, segment },
        },
  );
  return view.status === "current" ? view.value : { status: "pending" };
}
