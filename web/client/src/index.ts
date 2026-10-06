// The TypeScript client of the Cairn API (ARCHITECTURE, Repository layout: web/client): a
// typed fetch client over the types generated from the OpenAPI document. The safe retry
// and subscription wrapper (H5, H6) needs the engine's touched-set function through wasm,
// so it joins with the web shell (4.6).
import createClient, { type Client, type ClientOptions } from "openapi-fetch";

import type { components, operations, paths } from "../generated/api.ts";

export type { components, operations, paths };

/** A schema the API names, by its name. */
export type Schema<Name extends keyof components["schemas"]> = components["schemas"][Name];

/** A client of one Cairn server. */
export type CairnClient = Client<paths>;

/**
 * A client of the server at `baseUrl`. A browser signed in by its session cookie passes no
 * token; a script passes an agent token, sent as a bearer token.
 */
export function createCairnClient(
  baseUrl: string,
  token?: string,
  options: Omit<ClientOptions, "baseUrl" | "headers"> = {},
): CairnClient {
  const headers: Record<string, string> =
    token === undefined ? {} : { Authorization: `Bearer ${token}` };
  return createClient<paths>({ ...options, baseUrl, headers });
}

/** Whether a patch answer committed now, rather than from an earlier receipt (H5). */
export function appliedNow(answer: Schema<"PatchAnswer">): boolean {
  return answer.outcome === "applied";
}
