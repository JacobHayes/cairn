// C14: the destination pages' side of the proposal layer. Each host reads the proposal at the
// page's address, works out the destination as it would be after (the engine's, never assembled
// here), assembles the `Layer` the views, the band and the rail read, and hands the page the
// destination to draw: the journey after, the route after, or the identities after. A proposal
// that is over, or never was, still gets its page, with the band saying so.
import { useMemo, type ReactNode } from "react";
import { Link, useLocation } from "react-router";

import { pathTo, proposalsAddress } from "../address.ts";
import { errorWords, DERIVE, beforeWork, valueOf, type ReadSource } from "../data/keyed-read.ts";
import type { DomainDocument, Deployment, Route, Deriver } from "../data/host.ts";
import type { Domain, ProposalReview } from "../data/proposals.ts";
import { useDeployment, useJourney, useKeyedRead, useLive, useSession } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { indexPath, layerExit } from "../journeys/address.ts";
import { ROUTES_INDEX_PATH, routeLayerExit } from "../routes/address.ts";
import { ROUTE_GRAPH, routeGraphKey, type Shown } from "../screens/route-reads.ts";
import { Failed } from "../ui/Failed.tsx";
import { LayerContext, type Layer, type LayerMode } from "./layer.ts";
import { reviewKey, proposalReview } from "./read.ts";
import { reviewing as buildReviewing, slotOf, useEdits, useJourneyAfter, useLayerModel } from "./review.ts";

/** The proposal at the address, kept current: loading, failed, absent, or read. */
function useReview(destination: Domain, id: string) {
  const { view, refresh } = useLive(reviewKey(destination, id), proposalReview(destination, id));
  const review = valueOf(view);
  return { review, refresh, status: view.status, error: errorWords(view) };
}

/** What a page shows until the proposal is read, or when it cannot be. */
function Reading({ status, error, what }: { status: string; error: string | undefined; what: string }) {
  if (status === "failed") {
    return (
      <p className="callout callout-bad" role="alert">
        <Failed what={what} error={error ?? ""} />
      </p>
    );
  }
  return <p className="muted small">Reading the {what.toLowerCase()}...</p>;
}

/** The mode a proposal read is in: under review while open. */
function modeOf(review: ProposalReview | undefined): LayerMode {
  if (review === undefined) {
    return "missing";
  }
  const status = review.proposal.status;
  return status === "open" ? "reviewing" : status === "applied" ? "applied" : "discarded";
}

/** Whether applying the proposal creates its destination: it was drafted against revision 0 and is still open. */
export function createsIt(review: ProposalReview): boolean {
  return review.proposal.draft.destination_revision === 0 && review.proposal.status === "open" && review.proposal.destination !== "deployment";
}

const NONE: ReadonlySet<string> = new Set();

const IDENTITIES_PATH = pathTo("identities");

/** A layer for a proposal that is over or absent: the destination as it is, no marks, only the band and the closed rail. */
function plain(base: Pick<Layer, "destination" | "kind" | "id" | "exit" | "proposals">, review: ProposalReview | undefined, before: Pick<Layer, "before" | "beforeShown" | "deployment">): Layer {
  return { ...base, ...before, mode: modeOf(review), review, reviewing: undefined, model: undefined, creates: false, removed: NONE, drawnAs: "before" };
}

/** What the journey after needs from the engine's answer when the proposal creates the journey: the document, derived and held under the proposal's slot. */
interface CreatedHeld {
  after: Ready;
  free: () => void;
}

const CREATED_AFTER: ReadSource<{ deriver: Deriver; journey: string; proposal: string; document: DomainDocument }, CreatedHeld> = {
  policy: DERIVE,
  read: async ({ deriver, journey, proposal, document }, at) => {
    beforeWork(at);
    const slot = slotOf(journey, proposal);
    const loaded = await deriver.load(JSON.stringify(document), slot);
    let freed = false;
    const free = () => {
      if (!freed) {
        freed = true;
        void deriver.release(loaded.handle).catch(() => undefined);
      }
    };
    at.signal.addEventListener("abort", free, { once: true });
    try {
      beforeWork(at);
      const derived = await deriver.derived(slot);
      return { after: { status: "ready", journey: document.journey, inputs: document.inputs, key: loaded.key, derived }, free };
    } catch (thrown) {
      free();
      throw thrown;
    }
  },
  release(value) {
    value.free();
  },
};

/** The journey a proposal creates, derived beside nothing: the host answer's after document, and whether it is derived for the proposal as read now. */
function useCreatedAfter(id: string, review: ProposalReview, creating: boolean): { after: Ready | undefined; current: boolean } {
  const { deriver } = useSession();
  const after = review.after;
  const document = creating && after?.destination === "journey" ? after.document : undefined;
  const key = document === undefined ? undefined : { of: [id, review.proposal.id], freshness: [review.proposal.revision, JSON.stringify(document)], args: { deriver, journey: id, proposal: review.proposal.id, document } };
  const { view } = useKeyedRead(CREATED_AFTER, key);
  return { after: valueOf(view)?.after, current: view.status === "current" };
}

interface JourneyHostProps {
  id: string;
  proposal: string;
  /** The page, over the journey to draw: the journey after, or as it is when there is no after; none when there is nothing to draw. */
  children: (ready: Ready | undefined) => ReactNode;
}

/** A journey under a proposal that is under review. */
function JourneyReviewing({ id, review, refetch, before, children }: Omit<JourneyHostProps, "proposal"> & { review: ProposalReview; refetch: () => void; before: Ready | undefined }) {
  const { search } = useLocation();
  const deployment = useDeployment();
  const proposal = review.proposal;
  const creates = before === undefined;
  const edits = useEdits(review, refetch);
  const draft = edits.kept?.draft ?? proposal.draft;
  const local = useJourneyAfter(before, proposal, draft);
  const created = useCreatedAfter(id, review, creates);
  const after = creates ? created.after : local.value?.after;
  const known = after !== undefined;
  // Apply is held until the journey after is derived for the proposal as it stands (a created one's, or the local preview's).
  const previewed = creates ? created.current : local.current;
  const previewOf = local.value?.preview;
  const model = useLayerModel({
    review,
    draft,
    kept: edits.kept,
    local: previewOf,
    localError: local.error,
    previewed,
    before: before?.journey.graph,
    after: after?.journey.graph,
    deployment,
    today: before?.key.today,
    writing: edits.write.disabled,
  });
  const destination: Domain = { journey: id };
  const exit = creates ? indexPath() : layerExit({ journey: id }, search);
  const reviewingNow = buildReviewing(edits, review, model.preview, previewed && !local.gone ? proposal.revision : undefined);
  const removed = useMemo<ReadonlySet<string>>(() => new Set(Object.entries(model.marks).filter(([, mark]) => mark.kind === "removed").map(([key]) => key)), [model.marks]);
  const layer: Layer = {
    mode: "reviewing",
    destination,
    kind: "journey",
    id: proposal.id,
    review,
    reviewing: reviewingNow,
    model,
    before,
    beforeShown: undefined,
    deployment: undefined,
    creates,
    removed,
    drawnAs: known ? "after" : before === undefined ? "nothing" : "before",
    exit,
    proposals: proposalsAddress(destination),
  };
  return <LayerContext value={layer}>{children(after ?? before)}</LayerContext>;
}

/** The journey page's layer: the proposal at its address over the journey, or over the journey it would create. */
export function JourneyLayerHost({ id, proposal: pid, children }: JourneyHostProps) {
  const destination: Domain = { journey: id };
  const { search } = useLocation();
  const read = useReview(destination, pid);
  const journey = useJourney(id);
  const base = { destination, kind: "journey" as const, id: pid, exit: layerExit({ journey: id }, search), proposals: proposalsAddress(destination) };
  if (read.review === undefined && read.status !== "gone") {
    return <Reading status={read.status} error={read.error} what="The proposal" />;
  }
  if (journey.status === "failed") {
    return <p className="callout callout-bad"><Failed what="The journey" error={journey.message} /></p>;
  }
  if (journey.status === "skew") {
    return <p className="callout">This journey comes from a newer Cairn; reload to see it.</p>;
  }
  if (journey.status === "loading") {
    return <p className="muted small">Deriving the journey...</p>;
  }
  const review = read.review;
  if (journey.status === "missing") {
    if (review !== undefined && createsIt(review) && review.proposal.status === "open") {
      return <JourneyReviewing id={id} review={review} refetch={read.refresh} before={undefined}>{children}</JourneyReviewing>;
    }
    // A proposal that is over keeps its receipt where the journey is missing (discarded: it never created it).
    if (review !== undefined && review.proposal.status !== "open") {
      return <LayerContext value={plain(base, review, { before: undefined, beforeShown: undefined, deployment: undefined })}>{children(undefined)}</LayerContext>;
    }
    return (
      <p className="callout" data-testid="journey-missing">
        This journey does not exist. <Link to={indexPath()}>All journeys</Link>
      </p>
    );
  }
  if (review === undefined || review.proposal.status !== "open") {
    return <LayerContext value={plain(base, review, { before: journey, beforeShown: undefined, deployment: undefined })}>{children(journey)}</LayerContext>;
  }
  return <JourneyReviewing id={id} review={review} refetch={read.refresh} before={journey}>{children}</JourneyReviewing>;
}

interface RouteHostProps {
  id: string;
  proposal: string;
  /** The page, over the route to draw (the route after, or as it is); none when there is nothing to draw. */
  children: (shown: Shown | undefined, route: Route | undefined) => ReactNode;
}

/** The route as the proposal's after leaves it: the draft it holds, else its latest version; none when it holds no graph. */
export function shownAfter(after: ProposalReview["after"]): { shown: Shown | undefined; route: Route | undefined } {
  if (after?.destination !== "route") {
    return { shown: undefined, route: undefined };
  }
  const { route, latest } = after;
  if (route.draft != null) {
    return { shown: { route, graph: route.draft.graph, of: "draft" }, route };
  }
  return { shown: latest == null ? undefined : { route, graph: latest.graph, of: latest.version }, route };
}

/** A route under a proposal that is under review. */
function RouteReviewing({ id, review, refetch, before, children }: Omit<RouteHostProps, "proposal"> & { review: ProposalReview; refetch: () => void; before: Shown | undefined }) {
  const { search } = useLocation();
  const deployment = useDeployment();
  const proposal = review.proposal;
  const creates = createsIt(review);
  const edits = useEdits(review, refetch);
  const draft = edits.kept?.draft ?? proposal.draft;
  const { shown: after, route } = shownAfter(review.after);
  const known = after !== undefined;
  const model = useLayerModel({
    review,
    draft,
    kept: edits.kept,
    local: undefined,
    localError: undefined,
    previewed: true,
    before: before?.graph,
    after: after?.graph,
    deployment,
    today: undefined,
    writing: edits.write.disabled,
  });
  const destination: Domain = { route: id };
  const exit = creates ? ROUTES_INDEX_PATH : routeLayerExit(id, search);
  const reviewingNow = buildReviewing(edits, review, model.preview, proposal.revision);
  const removed = useMemo<ReadonlySet<string>>(() => new Set(Object.entries(model.marks).filter(([, mark]) => mark.kind === "removed").map(([key]) => key)), [model.marks]);
  const layer: Layer = {
    mode: "reviewing",
    destination,
    kind: "route",
    id: proposal.id,
    review,
    reviewing: reviewingNow,
    model,
    before: undefined,
    beforeShown: before,
    deployment: undefined,
    creates,
    removed,
    drawnAs: known ? "after" : before === undefined ? "nothing" : "before",
    exit,
    proposals: proposalsAddress(destination),
  };
  return <LayerContext value={layer}>{children(after ?? before, route ?? before?.route)}</LayerContext>;
}

/** The route page's layer: the proposal at its address over the route as it stands, or the one it would create. */
export function RouteLayerHost({ id, proposal: pid, children }: RouteHostProps) {
  const destination: Domain = { route: id };
  const { search } = useLocation();
  const { host } = useSession();
  const read = useReview(destination, pid);
  const beforeRead = useKeyedRead(ROUTE_GRAPH, routeGraphKey(host, id, undefined, [read.review?.proposal.status])).view;
  const found = valueOf(beforeRead);
  const before = found !== undefined && "shown" in found ? found.shown : undefined;
  const base = { destination, kind: "route" as const, id: pid, exit: routeLayerExit(id, search), proposals: proposalsAddress(destination) };
  if (read.review === undefined && read.status !== "gone") {
    return <Reading status={read.status} error={read.error} what="The proposal" />;
  }
  const review = read.review;
  const creating = review !== undefined && createsIt(review);
  if (found === undefined && beforeRead.status !== "gone") {
    return beforeRead.status === "failed" ? <p className="callout callout-bad" role="alert"><Failed what="The route" error={beforeRead.error} /></p> : <p className="muted small">Reading the route...</p>;
  }
  const over = review !== undefined && review.proposal.status !== "open";
  if (found === undefined && !creating && !over) {
    return <p className="callout">This route does not exist. <Link to={ROUTES_INDEX_PATH}>All routes</Link></p>;
  }
  if (review === undefined || review.proposal.status !== "open") {
    return <LayerContext value={plain(base, review, { before: undefined, beforeShown: before, deployment: undefined })}>{children(before, before?.route)}</LayerContext>;
  }
  return <RouteReviewing id={id} review={review} refetch={read.refresh} before={before}>{children}</RouteReviewing>;
}

interface DeploymentHostProps {
  proposal: string;
  /** The page, over the identities to list: the deployment after, or as it is. */
  children: (before: Deployment, after: Deployment | undefined) => ReactNode;
}

/** The identities under a proposal that is under review. */
function DeploymentReviewing({ review, refetch, before, children }: Omit<DeploymentHostProps, "proposal"> & { review: ProposalReview; refetch: () => void; before: Deployment }) {
  const edits = useEdits(review, refetch);
  const proposal = review.proposal;
  const model = useLayerModel({
    review,
    draft: edits.kept?.draft ?? proposal.draft,
    kept: edits.kept,
    local: undefined,
    localError: undefined,
    previewed: true,
    before: undefined,
    after: undefined,
    deployment: before,
    today: undefined,
    writing: edits.write.disabled,
  });
  const after = review.after?.destination === "deployment" ? review.after.deployment : undefined;
  const layer: Layer = {
    mode: "reviewing",
    destination: "deployment",
    kind: "deployment",
    id: proposal.id,
    review,
    reviewing: buildReviewing(edits, review, model.preview, proposal.revision),
    model,
    before: undefined,
    beforeShown: undefined,
    deployment: { before, after },
    creates: false,
    removed: NONE,
    drawnAs: after === undefined ? "before" : "after",
    exit: IDENTITIES_PATH,
    proposals: proposalsAddress("deployment"),
  };
  return <LayerContext value={layer}>{children(before, after)}</LayerContext>;
}

/** The deployment page's layer: the proposal at its address over the identities as they are. */
export function DeploymentLayerHost({ proposal: pid, children }: DeploymentHostProps) {
  const read = useReview("deployment", pid);
  const deployment = useDeployment();
  const review = read.review;
  if (review === undefined && read.status !== "gone") {
    return <Reading status={read.status} error={read.error} what="The proposal" />;
  }
  if (deployment === undefined) {
    return <p className="muted small">Reading the deployment...</p>;
  }
  if (review === undefined || review.proposal.status !== "open") {
    const base = { destination: "deployment" as const, kind: "deployment" as const, id: pid, exit: IDENTITIES_PATH, proposals: proposalsAddress("deployment") };
    return <LayerContext value={plain(base, review, { before: undefined, beforeShown: undefined, deployment: { before: deployment, after: undefined } })}>{children(deployment, undefined)}</LayerContext>;
  }
  return <DeploymentReviewing review={review} refetch={read.refresh} before={deployment}>{children}</DeploymentReviewing>;
}
