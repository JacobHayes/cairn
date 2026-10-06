// The host switch: which host a tab runs on. `?host=server` or `?host=browser` chooses;
// otherwise the server when one answers at the page's origin (the binary, or Vite's proxy to
// a running one), and the in-browser host when none does (a static demo site, or Vite with
// no server). The choice holds for the tab's life; switching reloads.
import type { HostKind } from "./host.ts";

/** The host the page's address asks for, if it asks. */
export function requestedHost(search: string): HostKind | undefined {
  const asked = new URLSearchParams(search).get("host");
  return asked === "server" || asked === "browser" ? asked : undefined;
}

/** The page's address with the other host asked for, its hash (the screen) kept. */
export function switchedTo(location: Pick<Location, "origin" | "pathname" | "search" | "hash">, kind: HostKind): string {
  const search = new URLSearchParams(location.search);
  search.set("host", kind);
  return `${location.origin}${location.pathname}?${search.toString()}${location.hash}`;
}

/** The host a tab runs on: the one asked for, or the server when one answers. */
export async function chooseHost(search: string, serverAnswers: () => Promise<boolean>): Promise<HostKind> {
  return requestedHost(search) ?? ((await serverAnswers()) ? "server" : "browser");
}
