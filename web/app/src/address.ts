// Every address pattern of the app, defined once (C16, C17, C14). The router registers its
// routes from `ADDRESSES`, every link is built with `pathTo` or one of the helpers below, and
// every reader that needs to know what a path names asks `addressOf`. Nothing else in the app
// writes or splits a path by hand. A proposal lives under its destination, so a proposal has
// no address of its own: `/journeys/<journey>/proposals/<proposal>`, `/routes/<route>/proposals/<proposal>`,
// or `/entities/proposals/<proposal>` for the deployment; each has the same `/nodes/<key>` below it.
import { generatePath, matchPath } from "react-router";

import type { Schema } from "@cairn/client";

/** The rail entry a page belongs to. */
export type Section = "home" | "journeys" | "routes" | "entities" | "me";

/** Each pattern, in the order they are tried: the static `new` pages before the ones with a parameter in their place. */
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
  { name: "routeDraft", path: "/routes/:route/draft", section: "routes" },
  { name: "routeDraftNode", path: "/routes/:route/draft/nodes/:key", section: "routes" },
  { name: "routeVersions", path: "/routes/:route/versions", section: "routes" },
  { name: "routeProposals", path: "/routes/:route/proposals", section: "routes" },
  { name: "routeProposal", path: "/routes/:route/proposals/:proposal", section: "routes" },
  { name: "routeProposalNode", path: "/routes/:route/proposals/:proposal/nodes/:key", section: "routes" },
  { name: "entities", path: "/entities", section: "entities" },
  { name: "deploymentProposals", path: "/entities/proposals", section: "entities" },
  { name: "deploymentProposal", path: "/entities/proposals/:proposal", section: "entities" },
  { name: "deploymentProposalNode", path: "/entities/proposals/:proposal/nodes/:key", section: "entities" },
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

/** What a path is: which pattern it matches, with its parameters, or none for a path that is no page. */
export interface Matched {
  name: AddressName;
  section: Section;
  params: Readonly<Record<string, string | undefined>>;
}

export function addressOf(pathname: string): Matched | undefined {
  const trimmed = pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname;
  for (const { name, path, section } of ADDRESSES) {
    const found = matchPath({ path, end: true }, trimmed);
    if (found !== null) {
      return { name, section, params: found.params };
    }
  }
  return undefined;
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

/** A proposal's review, under its destination, with a node of it open when `key` is given. */
export function proposalAddress(destination: Domain, proposal: string, key?: string): string {
  if (destination === "deployment") {
    return key === undefined ? pathTo("deploymentProposal", { proposal }) : pathTo("deploymentProposalNode", { proposal, key });
  }
  if ("journey" in destination) {
    const { journey } = destination;
    return key === undefined ? pathTo("journeyProposal", { journey, proposal }) : pathTo("journeyProposalNode", { journey, proposal, key });
  }
  const { route } = destination;
  return key === undefined ? pathTo("routeProposal", { route, proposal }) : pathTo("routeProposalNode", { route, proposal, key });
}

/** The destination the router's params name: the journey, the route, else the deployment (`/entities/proposals`). */
export function destinationIn(params: Readonly<Record<string, string | undefined>>): Domain {
  if (params.journey !== undefined) {
    return { journey: params.journey };
  }
  return params.route === undefined ? "deployment" : { route: params.route };
}

/** The destination a proposal address is under, and the proposal, or none when `matched` is no proposal's. */
export function proposalIn(matched: Matched): { destination: Domain; proposal: string; key: string | undefined } | undefined {
  const { params } = matched;
  switch (matched.name) {
    case "journeyProposal":
    case "journeyProposalNode":
      return { destination: { journey: required(params, "journey") }, proposal: required(params, "proposal"), key: params.key };
    case "routeProposal":
    case "routeProposalNode":
      return { destination: { route: required(params, "route") }, proposal: required(params, "proposal"), key: params.key };
    case "deploymentProposal":
    case "deploymentProposalNode":
      return { destination: "deployment", proposal: required(params, "proposal"), key: params.key };
    default:
      return undefined;
  }
}

/** The rail entry the page at `pathname` belongs to. */
export function sectionOf(pathname: string): Section | undefined {
  return addressOf(pathname)?.section;
}
