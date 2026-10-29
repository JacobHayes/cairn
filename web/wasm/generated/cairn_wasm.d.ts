// Generated from crates/wasm by wasm-bindgen ('mise run gen'); do not edit.
/* tslint:disable */
/* eslint-disable */

/**
 * The browser host's composition root.
 */
export class BrowserRoot {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * I6: applies proposal `id`: `request` is the JSON of a [`ProposalApply`]; the answer
     * is the JSON of a [`PatchAnswer`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`], the rejection among them.
     */
    applyProposal(destination: string, id: string, request: string, now: string): string;
    /**
     * The caller at `now`, as `GET /api/users/me` answers it (H3).
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    caller(now: string): string;
    /**
     * The capabilities document: one local sign-in, no assistant, MCP, or SSE.
     */
    capabilities(): string;
    /**
     * The deployment: its identities, aliases, and revision (E6).
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    deployment(): string;
    /**
     * I6: discards proposal `id`: `request` is the JSON of a [`ProposalStep`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`], the rejection among them.
     */
    discardProposal(destination: string, id: string, request: string, now: string): string;
    /**
     * A journey's domain document at `now`, as `GET /api/journeys/{id}/document` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such journey, or an unreadable input.
     */
    document(journey: string, now: string): string;
    /**
     * I6: edits proposal `id`: `request` is the JSON of a [`ProposalEdit`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`], the rejection among them.
     */
    editProposal(destination: string, id: string, request: string, now: string): string;
    /**
     * A13: version `version` of route `route` as a file, or its draft when `version` is
     * empty, as `GET /api/routes/{id}/export` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such route, version, or draft, or an unreadable input.
     */
    exportFile(route: string, version: string): string;
    /**
     * J4: a page of a journey's history, or of `node`'s (empty for the whole journey), after
     * the log position `after` (negative for the first page), as `GET /api/journeys/{id}/history`
     * answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such journey, or an unreadable input.
     */
    history(journey: string, node: string, after: number): string;
    /**
     * E6: what refers to identity `identity`, as
     * `GET /api/deployment/identities/{key}/references` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such identity, or an unreadable input.
     */
    identityReferences(identity: string): string;
    /**
     * A13: imports a route file as the local user at `now`: `request` is the JSON of a
     * [`RouteImport`]; the answer is the JSON of the patch answer, as
     * `POST /api/routes/{id}/import` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`], the rejection among them.
     */
    importFile(request: string, now: string): string;
    /**
     * C16: the journey index `query` (the JSON of a [`JourneyIndexQuery`]) asks for, as
     * `GET /api/journeys` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    journeyIndex(query: string): string;
    /**
     * The journey index (C16), as `GET /api/journeys` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    journeys(): string;
    /**
     * C14, C16: a page of open proposals: `request` is the JSON of a
     * [`ProposalListRequest`]; the answer is the JSON of a page, as the API's `ProposalList`.
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    listProposals(request: string, now: string): string;
    /**
     * A root with every fixture seeded.
     *
     * # Errors
     *
     * The JSON of a [`HostError`] when seeding fails.
     */
    constructor();
    /**
     * Submits a patch as the local user at `now`: `request` is the JSON of a
     * [`PatchRequest`]; the answer is the JSON of a [`PatchAnswer`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`], the rejection among them.
     */
    patch(request: string, now: string): string;
    /**
     * C14, I6: previews proposal `id` at `now`; the answer is the JSON of a
     * [`ProposalReview`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    previewProposal(destination: string, id: string, now: string): string;
    /**
     * I6: proposal `id`, as `GET /api/{destination}/proposals/{proposal}` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such proposal, or an unreadable input.
     */
    proposal(destination: string, id: string): string;
    /**
     * B9: proposes re-linking `journey`: `request` is the JSON of a [`RelinkRequest`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    proposeRelink(journey: string, request: string, now: string): string;
    /**
     * B8: proposes saving `journey` as a route: `request` is the JSON of a
     * [`SaveAsRouteRequest`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    proposeSaveAsRoute(journey: string, request: string, now: string): string;
    /**
     * B7: proposes upgrading `journey`: `request` is the JSON of an [`UpgradeRequest`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    proposeUpgrade(journey: string, request: string, now: string): string;
    /**
     * I6: creates a proposal for `destination` (the JSON of a `Domain`) at `now`: `request`
     * is the JSON of a [`ProposalCreate`]; the answer is the JSON of a [`ProposalAnswer`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`], the rejection among them.
     */
    propose(destination: string, request: string, now: string): string;
    /**
     * I6: refreshes proposal `id` on its destination as it stands: `request` is the JSON of
     * a [`ProposalStep`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`], the rejection among them.
     */
    refreshProposal(destination: string, id: string, request: string, now: string): string;
    /**
     * C17: route `route`'s versions with the journeys on each, as
     * `GET /api/routes/{id}/versions` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such route, or an unreadable input.
     */
    routeDetail(route: string): string;
    /**
     * One published version of a route (A11), as `GET /api/routes/{id}/versions/{version}`
     * answers it; `version` is its number as text.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such route or version, or an unreadable input.
     */
    routeVersion(route: string, version: string): string;
    /**
     * A route with its draft (A11), as `GET /api/routes/{id}` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such route, or an unreadable input.
     */
    route(route: string): string;
    /**
     * The route index of `kind` (the JSON of an optional kind: `null` for both) from after
     * `after` (empty for the first page), as `GET /api/routes` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    routes(after: string, kind: string): string;
    /**
     * H6: a subscriber to what `watching` (the JSON of a list of watch names) names, holding
     * the current revisions for its first take.
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    subscribe(watching: string): RootSubscription;
    /**
     * The names of the users in `users`, a JSON array of ids, as `GET /api/users` answers it
     * (H3).
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    userNames(users: string): string;
}

/**
 * A domain document with its journey validated and derived (D3): what every projection reads.
 */
export class Derivation {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * D3: every derived value in the schema's shape, as the server's derive answers it.
     */
    derived(): string;
    /**
     * What it was derived from, as a query cache keys it (ARCHITECTURE, Web UI): the
     * journey, its revision, the deployment revision, and today, as JSON.
     */
    key(): string;
    /**
     * Reads `document` and derives it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: an unreadable document or an invalid journey.
     */
    constructor(document: string);
    /**
     * One projection: `request` is the JSON of a [`Projection`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: an unreadable request, or a node the journey lacks.
     */
    project(request: string): string;
    /**
     * A10, G3: a message draft rendered: `request` is the JSON of a [`DraftRequest`]; the
     * answer is the JSON of a [`RenderedDraft`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: an unreadable request, or a node or draft the journey
     * lacks.
     */
    renderDraft(request: string): string;
}

/**
 * One subscriber of the in-browser root (H6). Freeing it unsubscribes.
 */
export class RootSubscription {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Takes what it holds at `now_ms` (milliseconds on a monotonic clock, as
     * `performance.now()` gives): the JSON of a [`Taken`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`] when `now_ms` is not a reading of a forward-running clock:
     * negative, not finite, too large for a duration, or earlier than one given before.
     */
    take(now_ms: number): string;
}

/**
 * A route's graph validated and derived as a journey that starts on day 0 and in which nothing
 * has happened (C2, C7, C13): what every projection of a route's canvas, list and timeline
 * reads.
 */
export class RouteDerivation {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Reads a [`RouteDeriveRequest`] and derives it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: an unreadable request or an invalid graph.
     */
    constructor(request: string);
    /**
     * One projection: `request` is the JSON of a [`Projection`].
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: an unreadable request, or a node the graph lacks.
     */
    project(request: string): string;
    /**
     * C13: the schedule, as the JSON of a `RouteSchedule`.
     */
    schedule(): string;
}

/**
 * Applies a draft patch to `document`'s journey without committing it: `request` is the JSON
 * of an [`ApplyRequest`]; the answer is the JSON of an [`AppliedLocally`].
 *
 * # Errors
 *
 * The JSON of a [`HostError`]: unreadable input, the rejection, or a patch to another domain.
 */
export function apply(document: string, request: string): string;

/**
 * Applies a draft patch to a route without committing it: `request` is the JSON of a
 * [`RouteApplyRequest`]; the answer is the JSON of the route as the patch leaves it.
 *
 * # Errors
 *
 * The JSON of a [`HostError`]: unreadable input, the rejection, or a patch to another domain.
 */
export function applyRoute(request: string): string;

/**
 * A5: the JSON of what keeps the clause (the JSON of a `Clause`) from being a condition: a
 * `{"too_large": {"limit", "count"}}`, `"empty_combination"`, `{"unreadable": {"reason"}}`, or
 * `null` when it can be one.
 */
export function conditionProblem(clause: string): string;

/**
 * A10: a draft an author wrote, stored with keys: `{"stored": text}`, or `{"problem":
 * {"unreadable": words}}` or `{"problem": {"unknown": [placeholders]}}`.
 *
 * # Errors
 *
 * The JSON of a [`HostError`] when the graph does not read.
 */
export function draftStored(graph: string, written: string): string;

/**
 * A10: a stored message draft as an author reads it (roles by id, decisions by path), as
 * the JSON of a string.
 *
 * # Errors
 *
 * The JSON of a [`HostError`] when the graph does not read.
 */
export function draftWritten(graph: string, stored: string): string;

/**
 * Exports a route version or draft: `request` is the JSON of an [`ExportRequest`]; the
 * answer is the JSON of the `RouteFile`, as `GET /api/routes/{id}/export` answers it.
 *
 * # Errors
 *
 * The JSON of a [`HostError`].
 */
export function exportRoute(request: string): string;

/**
 * Imports a route file into the graph its draft would hold: `request` is the JSON of an
 * [`ImportRequest`]; the answer is the JSON of the graph.
 *
 * # Errors
 *
 * The JSON of a [`HostError`].
 */
export function importRoute(request: string): string;

/**
 * A1a, A4, A9, G5: the JSON of the problems with a node form's values (the JSON of
 * [`NodeFormValues`]), one per field that cannot be sent, each `{"field", "message"}` with the
 * message in words for the person filling in the form; `[]` when all can be sent. The rules
 * are the types' own, the ones the patch reader builds a node from.
 *
 * # Errors
 *
 * The JSON of a [`HostError`] when the values do not read.
 */
export function nodeFormProblems(values: string): string;

/**
 * Previews a proposal against `document`'s journey: `request` is the JSON of a
 * [`PreviewRequest`]; the answer is the JSON of a `ProposalPreview`.
 *
 * # Errors
 *
 * The JSON of a [`HostError`]: unreadable input, or a proposal to another domain.
 */
export function preview(document: string, request: string): string;

/**
 * A13: a route file's text (YAML, or JSON, which is YAML) read as the file document, the
 * JSON the API's import takes: `GET` and `POST` carry JSON, disks hold YAML (ARCHITECTURE,
 * File format).
 *
 * # Errors
 *
 * The JSON of a [`HostError::Unreadable`] when the text is not a route file.
 */
export function readRouteFile(text: string): string;

/**
 * A18: the JSON of a `RemovalPlan`: what removing `target` (a `RemovalTarget`) from `graph`
 * (a `Graph`, with its state for a journey) takes and leaves dangling, with the mutations
 * that rewrite what dangles; `null` when a node asked for is not in the graph.
 *
 * # Errors
 *
 * The JSON of a [`HostError`] when either input does not read.
 */
export function removalPlan(graph: string, target: string): string;

/**
 * B4: the mutations that restore tombstoned node `key` of a route version (`version`, a
 * `Graph`) into a journey's graph, with fresh keys built from `seed`; `null` when the node
 * is not in the version or a message draft would not fit under the new keys.
 *
 * # Errors
 *
 * The JSON of a [`HostError`] when an input does not read.
 */
export function restoreNodes(journey: string, version: string, key: string, seed: string): string;

/**
 * A13: the file document (the JSON of a `RouteFile`, as an export answers it) written as
 * the canonical YAML a route file is kept in on disk: sorted, in a stable field order.
 *
 * # Errors
 *
 * The JSON of a [`HostError::Unreadable`] when the JSON is not a route file.
 */
export function routeFileText(file: string): string;

/**
 * A20: a route graph's notices: `request` is the JSON of a [`RouteNoticesRequest`]; the
 * answer is the JSON list of `Notice`s.
 *
 * # Errors
 *
 * The JSON of a [`HostError`]: an unreadable request or an invalid graph.
 */
export function routeNotices(request: string): string;

/**
 * Runs when the module is instantiated: a panic's message goes to the console, since the
 * abort that follows says only that the module trapped.
 */
export function start(): void;

/**
 * A1a, A6, A7, E6: the JSON of the problems with the loose values of a form (the JSON of
 * a list of [`ValueCheck`]), one `{"field", "message"}` per value that cannot be sent; `[]`
 * when all can be. The rules are the types' own, the ones the patch reader builds the same
 * values from.
 *
 * # Errors
 *
 * The JSON of a [`HostError`] when the checks do not read.
 */
export function valueProblems(checks: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_browserroot_free: (a: number, b: number) => void;
    readonly __wbg_derivation_free: (a: number, b: number) => void;
    readonly __wbg_rootsubscription_free: (a: number, b: number) => void;
    readonly __wbg_routederivation_free: (a: number, b: number) => void;
    readonly apply: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly applyRoute: (a: number, b: number, c: number) => void;
    readonly browserroot_applyProposal: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly browserroot_caller: (a: number, b: number, c: number, d: number) => void;
    readonly browserroot_capabilities: (a: number, b: number) => void;
    readonly browserroot_deployment: (a: number, b: number) => void;
    readonly browserroot_discardProposal: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly browserroot_document: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_editProposal: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly browserroot_exportFile: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_history: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly browserroot_identityReferences: (a: number, b: number, c: number, d: number) => void;
    readonly browserroot_importFile: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_journeyIndex: (a: number, b: number, c: number, d: number) => void;
    readonly browserroot_journeys: (a: number, b: number) => void;
    readonly browserroot_listProposals: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_new: (a: number) => void;
    readonly browserroot_patch: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_previewProposal: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly browserroot_proposal: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_propose: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly browserroot_proposeRelink: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly browserroot_proposeSaveAsRoute: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly browserroot_proposeUpgrade: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly browserroot_refreshProposal: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly browserroot_route: (a: number, b: number, c: number, d: number) => void;
    readonly browserroot_routeDetail: (a: number, b: number, c: number, d: number) => void;
    readonly browserroot_routeVersion: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_routes: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_subscribe: (a: number, b: number, c: number, d: number) => void;
    readonly browserroot_userNames: (a: number, b: number, c: number, d: number) => void;
    readonly conditionProblem: (a: number, b: number, c: number) => void;
    readonly derivation_derived: (a: number, b: number) => void;
    readonly derivation_key: (a: number, b: number) => void;
    readonly derivation_new: (a: number, b: number, c: number) => void;
    readonly derivation_project: (a: number, b: number, c: number, d: number) => void;
    readonly derivation_renderDraft: (a: number, b: number, c: number, d: number) => void;
    readonly draftStored: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly draftWritten: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly exportRoute: (a: number, b: number, c: number) => void;
    readonly importRoute: (a: number, b: number, c: number) => void;
    readonly nodeFormProblems: (a: number, b: number, c: number) => void;
    readonly preview: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly readRouteFile: (a: number, b: number, c: number) => void;
    readonly removalPlan: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly restoreNodes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly rootsubscription_take: (a: number, b: number, c: number) => void;
    readonly routeFileText: (a: number, b: number, c: number) => void;
    readonly routeNotices: (a: number, b: number, c: number) => void;
    readonly routederivation_new: (a: number, b: number, c: number) => void;
    readonly routederivation_project: (a: number, b: number, c: number, d: number) => void;
    readonly routederivation_schedule: (a: number, b: number) => void;
    readonly start: () => void;
    readonly valueProblems: (a: number, b: number, c: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
