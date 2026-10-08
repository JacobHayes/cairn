// The host switch: which host a tab runs on. `?host=server` or `?host=browser` chooses, and
// the tab keeps that choice in session storage, since the screens' own addresses rewrite the
// query; otherwise the server when one answers at the page's origin (the binary, or Vite's
// proxy to a running one), and the in-browser host when none does (a static demo site, or
// Vite with no server). The choice holds for the tab's life; switching reloads.
import type { HostKind } from "./host.ts";

const KEPT = "cairn:host";

function storage(): Storage | undefined {
  try {
    return globalThis.sessionStorage;
  } catch {
    return undefined;
  }
}

function hostKind(asked: string | null | undefined): HostKind | undefined {
  return asked === "server" || asked === "browser" ? asked : undefined;
}

/** The host the page's address asks for, if it asks. */
export function requestedHost(search: string): HostKind | undefined {
  return hostKind(new URLSearchParams(search).get("host"));
}

/** The host this tab was asked for earlier, if it was. */
export function keptHost(): HostKind | undefined {
  try {
    return hostKind(storage()?.getItem(KEPT));
  } catch {
    return undefined;
  }
}

/** Keeps `kind` as the tab's asked-for host. Storage that refuses keeps it for this load only. */
export function keepHost(kind: HostKind): void {
  try {
    storage()?.setItem(KEPT, kind);
  } catch {
    // This load only.
  }
}

/** `search` without the host switch's parameter, the rest as written: the screen's own query. */
export function withoutHost(search: string): string {
  const rest = search
    .replace(/^\?/, "")
    .split("&")
    .filter((pair) => pair !== "" && new URLSearchParams(pair).get("host") === null);
  return rest.length === 0 ? "" : `?${rest.join("&")}`;
}

/** The address of the screen at `pathname` and `search` with the other host asked for. */
export function switchedTo(origin: string, at: { pathname: string; search: string }, kind: HostKind): string {
  const search = new URLSearchParams(at.search);
  search.set("host", kind);
  return `${origin}${at.pathname}?${search.toString()}`;
}

/** The host a tab runs on: the one asked for now or before, or the server when one answers. */
export async function chooseHost(asked: HostKind | undefined, serverAnswers: () => Promise<boolean>): Promise<HostKind> {
  return asked ?? ((await serverAnswers()) ? "server" : "browser");
}
