// The review state of a proposal under the layer (C14, I6): the reviewer's edits kept in the tab
// until saved, the writes that save, refresh, apply and discard, and the model the views and the
// rail read, worked out from the proposal as edited and the destination before and after.
import type { Schema } from "@cairn/client";
import { useCallback, useMemo, useState } from "react";

import type { Graph } from "../authoring/graph.ts";
import { PREVIEW_SETTLE_MS } from "../authoring/write.ts";
import { useDraft } from "../data/drafts.ts";
import type { Deployment, Deriver, Host, RouteVersion } from "../data/host.ts";
import { beforeWork, previewPolicy, valueOf, type ReadSource } from "../data/keyed-read.ts";
import type { ProposalReview } from "../data/proposals.ts";
import { useKeyedRead, useSession, useToday, useCaller } from "../data/react.ts";
import { isPatchAnswer } from "../data/writes.ts";
import type { Ready } from "../detail/model.ts";
import { addedRoles, insertionMembers } from "../segments/model.ts";
import { useSegmentNames } from "../segments/read.ts";
import { applyRevision, layerMarks, type Kept, type LayerModel, type ProposalOutline, type Reviewing } from "./layer.ts";
import { applyBlockers, type DiffMark, type Proposal, type ProposalDraft, type ProposalPreview, type UnresolvedReason } from "./model.ts";
import { namesOf } from "./words.ts";
import { useProposalWrite } from "./write.ts";

/** The editors holding a value with a problem, by name, and how one reports it. */
function useInvalidEditors() {
  const [invalid, setInvalidSet] = useState<ReadonlySet<string>>(new Set());
  const setInvalid = useCallback((editor: string, held: boolean) => {
    setInvalidSet((current) => {
      if (current.has(editor) === held) {
        return current;
      }
      const next = new Set(current);
      if (held) {
        next.add(editor);
      } else {
        next.delete(editor);
      }
      return next;
    });
  }, []);
  return { invalid, setInvalid };
}

/** The reviewer's edits to a proposal and the writes that carry them (I6): kept in the tab until saved. */
export function useEdits(review: ProposalReview, refetch: () => void) {
  const proposal = review.proposal;
  const id = proposal.id;
  const write = useProposalWrite(`proposal:${id}`, proposal.draft.title);
  const [kept, setKept] = useDraft<Kept>(`proposal:${id}`);
  const { invalid, setInvalid } = useInvalidEditors();
  /** Reads the proposal again once a write landed, if the review is still there to read it. */
  const reread = () => {
    if (write.mounted()) {
      refetch();
    }
  };
  return { write, kept, setKept, editing: invalid.size > 0, setInvalid, reread };
}

export type Edits = ReturnType<typeof useEdits>;

/**
 * I6: the review's actions over the edits. Apply sends the revision the layer has drawn, when
 * that is the latest (`applyRevision`). Once a write landed the page is re-read and shows the proposal as
 * applied or discarded, under the same address, with the band to Close it.
 */
export function reviewing(edits: Edits, review: ProposalReview, preview: ProposalPreview | undefined, drawn: number | undefined): Reviewing {
  const { write, kept, setKept, editing, setInvalid, reread } = edits;
  const proposal = review.proposal;
  const id = proposal.id;
  const done = () => {
    reread();
  };
  return {
    write,
    kept,
    editing,
    setInvalid,
    blockers: applyBlockers({ proposal, dirty: kept !== undefined, editing, stale: review.stale != null, preview: drawn === undefined ? undefined : preview }),
    edit: (next) => {
      setKept({ base: kept?.base ?? proposal.revision, draft: next });
    },
    drop: () => {
      setKept(undefined);
    },
    /** H5: the reviewer's edits kept, to be saved over the proposal as it now stands. */
    rebase: () => {
      if (kept !== undefined) {
        setKept({ base: proposal.revision, draft: kept.draft });
      }
      write.dismiss();
    },
    save: () => {
      if (kept !== undefined && !editing) {
        void write.run({ op: "edit", destination: proposal.destination, proposal: id, request: { base_revision: kept.base, draft: kept.draft } }).then((answer) => {
          if (answer !== undefined) {
            setKept(undefined);
          }
          if (answer !== undefined && !isPatchAnswer(answer)) {
            reread();
          }
        });
      }
    },
    refresh: () => {
      void write.run({ op: "refresh", destination: proposal.destination, proposal: id, request: { base_revision: proposal.revision } }).then((answer) => {
        if (answer !== undefined && !isPatchAnswer(answer)) {
          reread();
        }
      });
    },
    apply: () => {
      const revision = applyRevision(proposal.revision, drawn);
      if (revision !== undefined) {
        void write.run({ op: "apply", destination: proposal.destination, proposal: id, request: { reviewed_revision: revision } }).then(done);
      }
    },
    discard: () => {
      void write.run({ op: "discard", destination: proposal.destination, proposal: id, request: { base_revision: proposal.revision } }).then(done);
    },
  };
}

/** The route versions a journey proposal's mutations read: an upgrade's target and the journey's own, a re-link's new version, an insertion's segment. */
function versionsRead(ready: Ready, draft: ProposalDraft): Schema<"Lineage">[] {
  const own = ready.journey.header.lineage ?? undefined;
  return (draft.mutations ?? []).flatMap((mutation) => {
    if (mutation.op === "upgrade" && own !== undefined) {
      return [own, { route: own.route, version: mutation.to }];
    }
    if (mutation.op === "relink") {
      return own === undefined ? [mutation.lineage] : [own, mutation.lineage];
    }
    return mutation.op === "insert_segment" ? [mutation.segment] : [];
  });
}

/** What a journey proposal's local preview holds: the engine's answer for the edited proposal, and the journey it would leave, derived and held under the proposal's slot. */
export interface JourneyAfter {
  preview: ProposalPreview;
  /** The journey as it would be, derived; none when the candidate breaks a rule. */
  after: Ready | undefined;
  /** Frees the worker's derivation. */
  free: () => void;
}

/** What the local preview is made from: the journey as derived, the proposal and its edits, and who previews. */
interface AfterArgs {
  host: Host;
  deriver: Deriver;
  before: Ready;
  proposal: Proposal;
  draft: ProposalDraft;
  user: string;
}

let previews = 0;

/**
 * A slot of its own, in the derive worker, for each preview of a proposal's journey: what is read
 * through one preview's key is never answered from the one that replaced it.
 */
export const slotOf = (journey: string, proposal: string): string => {
  previews += 1;
  return `${journey}/proposals/${proposal}#${String(previews)}`;
};

/**
 * C14: the edited proposal previewed by the engine over the journey the derive worker holds, and
 * the journey it leaves derived beside it under the proposal's slot; the engine's refusal is the
 * read's failure. The handle is released when the read drops the value.
 */
const JOURNEY_AFTER: ReadSource<AfterArgs, JourneyAfter> = {
  policy: previewPolicy(PREVIEW_SETTLE_MS),
  read: async ({ host, deriver, before, proposal, draft, user }, at) => {
    const lineages = versionsRead(before, draft);
    const versions: RouteVersion[] = await Promise.all(lineages.map((lineage) => host.routeVersion(lineage.route, lineage.version)));
    // B13: the engine reads a segment's header for its kind and retirement.
    const segments = await Promise.all([...new Set((draft.mutations ?? []).flatMap((mutation) => (mutation.op === "insert_segment" ? [mutation.segment.route] : [])))].map((route) => host.route(route)));
    // The edits may have moved on while the versions were out: obsolete work posts nothing.
    beforeWork(at);
    const journey = before.journey.header.id;
    const slot = slotOf(journey, proposal.id);
    const answer = await deriver.preview(before.key, { proposal: { ...proposal, draft }, versions, segments, at: new Date().toISOString(), actor: { user } }, slot);
    const handle = answer.loaded?.handle;
    let freed = false;
    const free = () => {
      if (!freed && handle !== undefined) {
        freed = true;
        void deriver.release(handle).catch(() => undefined);
      }
    };
    at.signal.addEventListener("abort", free, { once: true });
    try {
      beforeWork(at);
      const destination = answer.after;
      if (destination?.destination === "journey" && answer.loaded !== undefined) {
        const derived = await deriver.derived(slot);
        const { document } = destination;
        const after: Ready = { status: "ready", journey: document.journey, inputs: document.inputs, key: answer.loaded.key, derived };
        return { preview: answer.preview, after, free };
      }
      return { preview: answer.preview, after: undefined, free };
    } catch (thrown) {
      free();
      throw thrown;
    }
  },
  release(value) {
    value.free();
  },
};

/** The read of a journey proposal's preview: what it holds, its failure, and whether it is of the edits now on screen. */
export function useJourneyAfter(before: Ready | undefined, proposal: Proposal, draft: ProposalDraft) {
  const { deriver, host } = useSession();
  const { caller } = useCaller();
  const user = caller?.user ?? "u_local";
  // Only an open proposal is previewed here: one applied or discarded has nothing left to apply.
  const key =
    before === undefined || proposal.status !== "open"
      ? undefined
      : {
          of: [before.journey.header.id, proposal.id],
          freshness: [JSON.stringify(before.key), proposal.revision, JSON.stringify(draft), user],
          args: { host, deriver, before, proposal, draft, user },
        };
  const { view } = useKeyedRead(JOURNEY_AFTER, key);
  const value = valueOf(view);
  return {
    value,
    /** The read is of the proposal and edits on screen now, not the earlier ones it is replacing. */
    current: view.status === "current",
    error: view.status === "failed" || view.status === "behind" ? view.error.message : undefined,
    gone: view.status === "gone",
  };
}

/** What the model is made from. */
export interface ModelArgs {
  review: ProposalReview;
  /** The edits on screen, or the proposal as saved. */
  draft: ProposalDraft;
  kept: Kept | undefined;
  /** The preview of the edits (a journey's), or none, the host's being the proposal as saved. */
  local: ProposalPreview | undefined;
  localError: string | undefined;
  /** The preview is of what is on screen now. */
  previewed: boolean;
  before: Graph | undefined;
  after: Graph | undefined;
  /** The graphs the names are read from, besides these two. */
  deployment: Deployment | undefined;
  today: string | undefined;
  writing: boolean;
}

/** What a review shows, worked out from the proposal as edited and the graphs before and after it. */
export function useLayerModel(args: ModelArgs): LayerModel {
  const { review, draft, kept, local, localError, previewed, before, after, deployment, writing } = args;
  const proposal = review.proposal;
  const sessionToday = useToday();
  // A journey's preview follows the edits; any other's is the host's, of the proposal as saved.
  const previewOfEdits = local !== undefined;
  const preview = local ?? review.preview;
  // A candidate that breaks a rule has no destination after; that is not an empty graph, so nothing is compared.
  const known = after !== undefined;
  const nameOf = useSegmentNames();
  const members = useMemo(() => insertionMembers(before, after, nameOf), [before, after, nameOf]);
  const current = previewOfEdits || kept === undefined;
  const unresolved = useMemo(() => (current ? new Map((preview.unresolved ?? []).map((item) => [item.item, item.reason])) : new Map<number, UnresolvedReason>()), [current, preview.unresolved]);
  // What a conflict offers is the conflict's own, which the reviewer's edits do not change.
  const conflicts = useMemo(() => new Map((preview.conflicts ?? []).map((review) => [review.item, review])), [preview.conflicts]);
  const open = useMemo(() => new Set(unresolved.keys()), [unresolved]);
  const marked = useMemo(() => layerMarks({ before, after, items: draft.items ?? [], unresolved: open, known, members }), [before, after, draft.items, open, known, members]);
  const added = useMemo<Graph>(() => ({ nodes: (draft.mutations ?? []).flatMap((mutation) => (mutation.op === "add_node" ? [mutation.node] : [])) }), [draft.mutations]);
  const today = args.today ?? sessionToday;
  const names = useMemo(() => namesOf([added, before, after], deployment ?? { revision: 0 }, today), [added, before, after, deployment, today]);
  const outline = useMemo<ProposalOutline>(
    () => (known ? { ...preview.outline, nodes_added: markedCount(marked.marks, "added"), nodes_changed: markedCount(marked.marks, "changed"), nodes_removed: markedCount(marked.marks, "removed") } : preview.outline),
    [known, preview.outline, marked.marks],
  );
  return {
    draft,
    known,
    preview,
    outline,
    previewOfEdits,
    previewError: localError,
    previewed,
    before,
    after,
    diff: marked.diff,
    marks: marked.marks,
    entries: marked.entries,
    names,
    unresolved,
    conflicts,
    today,
    editable: proposal.status === "open" && !writing,
  };
}

/** The insertions and roles a proposal adds, named for the rail (C19). */
export function rolesAddedBy(model: LayerModel) {
  return addedRoles(model.before, model.after);
}

/** How many nodes the layer marks with one kind of change: the count the rail's key shows, so its sentence says the same. */
function markedCount(marks: Record<string, DiffMark>, kind: DiffMark["kind"]): number {
  return Object.values(marks).filter((mark) => mark.kind === kind).length;
}
