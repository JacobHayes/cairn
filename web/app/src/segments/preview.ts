// ARCHITECTURE, Web UI: previews. The graph an insertion would leave, from the tab's own
// engine over the graph as held (committing nothing), so the stepper draws the incoming root
// where review will; or why the engine would refuse it. Debounced, so a name being typed
// is not asked for on every keystroke.
import { HostFailure } from "@cairn/wasm";
import { useEffect, useState } from "react";

import type { Graph, Mutation } from "../authoring/graph.ts";
import type { Authored } from "../authoring/target.ts";
import { useSession, useViewer } from "../data/react.ts";
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

/** The graph `authored` would hold after `mutation` inserts `segment`, or why not; pending while it settles. */
export function useCandidate(authored: Authored, mutation: Mutation | undefined, segment: SegmentVersion | undefined): Candidate {
  const { deriver } = useSession();
  const { viewer } = useViewer();
  const asked = mutation === undefined || segment === undefined ? "" : JSON.stringify([authored.target, authored.revision, mutation, segment.version.version]);
  const [answered, setAnswered] = useState<{ asked: string; candidate: Candidate } | undefined>(undefined);
  useEffect(() => {
    if (mutation === undefined || segment === undefined) {
      return undefined;
    }
    let live = true;
    const timer = setTimeout(() => {
      const patch = { id: newPatchId(), target: authored.target, base_revision: authored.revision, mutations: [mutation] };
      const actor = { user: viewer?.user ?? "u_local" };
      const at = new Date().toISOString();
      const read = async (): Promise<Graph> => {
        if ("journey" in authored.target) {
          const applied = await deriver.apply(authored.target.journey, { patch, at, actor, versions: [segment.version], segments: [segment.segment] });
          return applied.document.journey.graph;
        }
        const route = await deriver.applyRoute({ patch, at, actor, today: authored.today, deployment: authored.deployment, versions: [segment.version], segments: [segment.segment], ...(authored.route === undefined ? {} : { route: authored.route }) });
        return route.draft?.graph ?? authored.graph;
      };
      read().then(
        (after) => {
          if (live) {
            setAnswered({ asked, candidate: { status: "placed", before: authored.graph, after } });
          }
        },
        (thrown: unknown) => {
          if (live) {
            setAnswered({ asked, candidate: { status: "refused", reasons: reasonsOf(thrown) } });
          }
        },
      );
    }, SETTLE_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // `asked` names the target as held, the mutation and the segment version.
  }, [deriver, asked]);
  return answered?.asked === asked && asked !== "" ? answered.candidate : { status: "pending" };
}
