// Where the person is, as the tour needs it: Home, or a journey and its open node. Read from the
// address by the app's own patterns (address.ts), so nothing here splits a path by hand.
import { matchPath } from "react-router";

import { ADDRESSES } from "../address.ts";

export interface Location {
  /** On Home. */
  home: boolean;
  /** The journey the page is under, and the node open in it. */
  journey: string | undefined;
  node: string | undefined;
}

function pattern(name: "home" | "journey" | "journeyNode"): string {
  return ADDRESSES.find((each) => each.name === name)?.path ?? "";
}

/** The place `pathname` names. */
export function locate(pathname: string): Location {
  const node = matchPath({ path: pattern("journeyNode"), end: true }, pathname);
  const journey = matchPath({ path: pattern("journey"), end: true }, pathname);
  return {
    home: matchPath({ path: pattern("home"), end: true }, pathname) !== null,
    journey: node?.params["journey"] ?? journey?.params["journey"],
    node: node?.params["key"],
  };
}

/** Whether `pathname` is under journey `run`: its page, or a node, edge or proposal in it. */
export function underJourney(pathname: string, run: string | undefined): boolean {
  return run !== undefined && matchPath({ path: pattern("journey"), end: false }, pathname)?.params["journey"] === run;
}
