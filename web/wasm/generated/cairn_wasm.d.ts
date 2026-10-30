// Generated from crates/wasm by wasm-bindgen ('mise run gen'); do not edit.
/* tslint:disable */
/* eslint-disable */

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
 * B13, C19: the JSON of an `InsertionPlan`: where the roles and kinds of `route` (a `Graph`,
 * a published version's) land when inserted into `host` (a `Graph`, with its state for a
 * journey), given what `ask` (an `InsertionAsk`) has chosen, which nodes are left out and
 * which roots are placed.
 *
 * # Errors
 *
 * The JSON of a [`HostError`] when an input does not read.
 */
export function insertionPlan(host: string, route: string, ask: string): string;

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
    readonly __wbg_derivation_free: (a: number, b: number) => void;
    readonly __wbg_routederivation_free: (a: number, b: number) => void;
    readonly apply: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly applyRoute: (a: number, b: number, c: number) => void;
    readonly conditionProblem: (a: number, b: number, c: number) => void;
    readonly derivation_derived: (a: number, b: number) => void;
    readonly derivation_key: (a: number, b: number) => void;
    readonly derivation_new: (a: number, b: number, c: number) => void;
    readonly derivation_project: (a: number, b: number, c: number, d: number) => void;
    readonly derivation_renderDraft: (a: number, b: number, c: number, d: number) => void;
    readonly draftStored: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly draftWritten: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly insertionPlan: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly nodeFormProblems: (a: number, b: number, c: number) => void;
    readonly preview: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly readRouteFile: (a: number, b: number, c: number) => void;
    readonly removalPlan: (a: number, b: number, c: number, d: number, e: number) => void;
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
