// Every address pattern of the app, defined once (C16, C17, C14). The router registers its
// routes from `ADDRESSES` and tells each page which pattern it matched (`usePage`), every link
// is built with `pathTo` or one of the helpers below, and the rail asks `sectionOf`. Nothing
// else in the app writes or splits a path by hand. A proposal lives under its destination, so a proposal has
// no address of its own: `/journeys/<journey>/proposals/<proposal>`, `/routes/<route>/proposals/<proposal>`,
// or `/identities/proposals/<proposal>` for the deployment. Each is the destination's own page with the
// proposal layer on (proposals/layer.ts); a journey's and a route's have `/nodes/<key>` below them, the
// deployment has no nodes.
import { createContext, useContext, useMemo } from "react";
import { generatePath, matchPath, useParams } from "react-router";

import type { Schema } from "@cairn/client";

/** The rail entry a page belongs to. */
export type Section = "home" | "journeys" | "routes" | "identities" | "me";

/** Each pattern; the router ranks the static pages (`new`, `proposals`) above the ones with a parameter in their place: no identity key is `new` or `proposals`, for keys start `who_`. */
export const ADDRESSES = [
  { name: "home", path: "/", section: "home" },
  { name: "journeys", path: "/journeys", section: "journeys" },
  { name: "journeyNew", path: "/journeys/new", section: "journeys" },
  { name: "journey", path: "/journeys/:journey", section: "journeys" },
  { name: "journeyNode", path: "/journeys/:journey/nodes/:key", section: "journeys" },
  { name: "journeyEdge", path: "/journeys/:journey/edges/:edge", section: "journeys" },
  { name: "journeyProposals", path: "/journeys/:journey/proposals", section: "journeys" },
  { name: "journeyProposal", path: "/journeys/:journey/proposals/:proposal", section: "journeys" },
  { name: "journeyProposalNode", path: "/journeys/:journey/proposals/:proposal/nodes/:key", section: "journeys" },
  { name: "routes", path: "/routes", section: "routes" },
  { name: "routeNew", path: "/routes/new", section: "routes" },
  { name: "route", path: "/routes/:route", section: "routes" },
  { name: "routeNode", path: "/routes/:route/nodes/:key", section: "routes" },
  { name: "routeProposals", path: "/routes/:route/proposals", section: "routes" },
  { name: "routeProposal", path: "/routes/:route/proposals/:proposal", section: "routes" },
  { name: "routeProposalNode", path: "/routes/:route/proposals/:proposal/nodes/:key", section: "routes" },
  { name: "identities", path: "/identities", section: "identities" },
  { name: "newIdentity", path: "/identities/new", section: "identities" },
  { name: "identity", path: "/identities/:key", section: "identities" },
  { name: "deploymentProposals", path: "/identities/proposals", section: "identities" },
  { name: "deploymentProposal", path: "/identities/proposals/:proposal", section: "identities" },
  { name: "me", path: "/me", section: "me" },
] as const;

export type AddressName = (typeof ADDRESSES)[number]["name"];

type Entry<N extends AddressName> = Extract<(typeof ADDRESSES)[number], { name: N }>;

/** The parameter names in a pattern's path. */
type ParamsOf<P extends string> = P extends `${string}:${infer Name}/${infer Rest}` ? Name | ParamsOf<`/${Rest}`> : P extends `${string}:${infer Name}` ? Name : never;

/** The parameters pattern `N` takes. */
export type AddressParams<N extends AddressName> = Record<ParamsOf<Entry<N>["path"]>, string>;

function entryOf(name: AddressName) {
  const found = ADDRESSES.find((each) => each.name === name);
  if (found === undefined) {
    throw new Error(`no address pattern ${name}`);
  }
  return found;
}

/** The path of pattern `name` with its parameters. */
export function pathTo<N extends AddressName>(name: N, ...params: keyof AddressParams<N> extends never ? [] : [AddressParams<N>]): string {
  return generatePath(entryOf(name).path, (params[0] ?? {}) as Record<string, string>);
}

/** The page being rendered: the pattern the router matched for it, with its parameters. */
export interface Page {
  name: AddressName;
  params: Readonly<Record<string, string | undefined>>;
}

/** The pattern the router matched, given to each page by its route (main.tsx); none outside a page. */
export const AddressContext = createContext<AddressName | undefined>(undefined);

/** The page being rendered, as the router matched it, or none outside a page's route. */
export function usePage(): Page | undefined {
  const name = useContext(AddressContext);
  const params = useParams();
  return useMemo(() => (name === undefined ? undefined : { name, params }), [name, params]);
}

/** A parameter an address of this pattern always has; reading one a pattern lacks is a bug. */
export function required(params: Readonly<Record<string, string | undefined>>, name: string): string {
  const value = params[name];
  if (value === undefined) {
    throw new Error(`the address has no ${name}`);
  }
  return value;
}

type Domain = Schema<"Domain">;

/** The proposals waiting at a destination: its inspector tab opens on `/proposals`. */
export function proposalsAddress(destination: Domain): string {
  if (destination === "deployment") {
    return pathTo("deploymentProposals");
  }
  return "journey" in destination ? pathTo("journeyProposals", { journey: destination.journey }) : pathTo("routeProposals", { route: destination.route });
}

/** A proposal's layer over its destination's page, with a node of it picked when `key` is given (the deployment has no nodes). */
export function proposalAddress(destination: Domain, proposal: string, key?: string): string {
  if (destination === "deployment") {
    return pathTo("deploymentProposal", { proposal });
  }
  if ("journey" in destination) {
    const { journey } = destination;
    return key === undefined ? pathTo("journeyProposal", { journey, proposal }) : pathTo("journeyProposalNode", { journey, proposal, key });
  }
  const { route } = destination;
  return key === undefined ? pathTo("routeProposal", { route, proposal }) : pathTo("routeProposalNode", { route, proposal, key });
}

/** The destination the router's params name: the journey, the route, else the deployment (`/identities/proposals`). */
export function destinationIn(params: Readonly<Record<string, string | undefined>>): Domain {
  if (params.journey !== undefined) {
    return { journey: params.journey };
  }
  return params.route === undefined ? "deployment" : { route: params.route };
}

/** The rail entry the page at `pathname` belongs to, or none for a path that is no page. */
export function sectionOf(pathname: string): Section | undefined {
  return ADDRESSES.find(({ path }) => matchPath({ path, end: true }, pathname) !== null)?.section;
}
