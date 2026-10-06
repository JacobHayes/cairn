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
     * The capabilities document: one local sign-in, no assistant, MCP, or SSE.
     */
    capabilities(): string;
    /**
     * The deployment: its entities, aliases, and revision (E6).
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    deployment(): string;
    /**
     * A journey's domain document at `now`, as `GET /journeys/{id}/document` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such journey, or an unreadable input.
     */
    document(journey: string, now: string): string;
    /**
     * J4: a page of a journey's history, or of `node`'s (empty for the whole journey), after
     * the log position `after` (negative for the first page), as `GET /journeys/{id}/history`
     * answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: no such journey, or an unreadable input.
     */
    history(journey: string, node: string, after: number): string;
    /**
     * The journey index (C16), as `GET /journeys` answers it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    journeys(): string;
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
     * H6: a subscriber to what `watching` (the JSON of a list of watch names) names, holding
     * the current revisions for its first take.
     *
     * # Errors
     *
     * The JSON of a [`HostError`].
     */
    subscribe(watching: string): RootSubscription;
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
     * Reads `document` (refusing another engine version's) and derives it.
     *
     * # Errors
     *
     * The JSON of a [`HostError`]: version skew, an unreadable document, or an invalid
     * journey.
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
 * Applies a draft patch to `document`'s journey without committing it: `request` is the JSON
 * of an [`ApplyRequest`]; the answer is the JSON of an [`AppliedLocally`].
 *
 * # Errors
 *
 * The JSON of a [`HostError`]: version skew, unreadable input, the rejection, or a patch to
 * another domain.
 */
export function apply(document: string, request: string): string;

/**
 * The engine version this module was built from: what a host compares a document's with
 * before deriving, previewing, or writing with it.
 */
export function engineVersion(): string;

/**
 * Exports a route version or draft: `request` is the JSON of an [`ExportRequest`]; the
 * answer is the JSON of the `RouteFile`, as `GET /routes/{id}/export` answers it.
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
 * Previews a proposal against `document`'s journey: `request` is the JSON of a
 * [`PreviewRequest`]; the answer is the JSON of a `ProposalPreview`.
 *
 * # Errors
 *
 * The JSON of a [`HostError`]: version skew, unreadable input, or a proposal to another
 * domain.
 */
export function preview(document: string, request: string): string;

/**
 * Runs when the module is instantiated: a panic's message goes to the console, since the
 * abort that follows says only that the module trapped.
 */
export function start(): void;

/**
 * H5: what `patch` (its JSON) touches, as the JSON of a `TouchedSet`.
 *
 * # Errors
 *
 * The JSON of a [`HostError`] when the patch does not read.
 */
export function touched(patch: string): string;

/**
 * H5: whether what intervened (the JSON of a stale rejection's `TouchedSet`) overlaps what
 * `patch` touches, so an automatic resubmission is not safe.
 *
 * # Errors
 *
 * The JSON of a [`HostError`] when either input does not read.
 */
export function touchedOverlaps(patch: string, intervening: string): boolean;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_browserroot_free: (a: number, b: number) => void;
    readonly __wbg_derivation_free: (a: number, b: number) => void;
    readonly __wbg_rootsubscription_free: (a: number, b: number) => void;
    readonly apply: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly browserroot_capabilities: (a: number, b: number) => void;
    readonly browserroot_deployment: (a: number, b: number) => void;
    readonly browserroot_document: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_history: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly browserroot_journeys: (a: number, b: number) => void;
    readonly browserroot_new: (a: number) => void;
    readonly browserroot_patch: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserroot_subscribe: (a: number, b: number, c: number, d: number) => void;
    readonly derivation_derived: (a: number, b: number) => void;
    readonly derivation_key: (a: number, b: number) => void;
    readonly derivation_new: (a: number, b: number, c: number) => void;
    readonly derivation_project: (a: number, b: number, c: number, d: number) => void;
    readonly derivation_renderDraft: (a: number, b: number, c: number, d: number) => void;
    readonly engineVersion: (a: number) => void;
    readonly exportRoute: (a: number, b: number, c: number) => void;
    readonly importRoute: (a: number, b: number, c: number) => void;
    readonly preview: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly rootsubscription_take: (a: number, b: number, c: number) => void;
    readonly start: () => void;
    readonly touched: (a: number, b: number, c: number) => void;
    readonly touchedOverlaps: (a: number, b: number, c: number, d: number, e: number) => void;
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
