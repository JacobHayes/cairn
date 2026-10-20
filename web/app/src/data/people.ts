// H3: the names people go by, for the user ids an event, a proposal or a note carries. The host
// names a batch of users in one read (never one per row): a screen asks for the names it shows
// as it draws, the asks of one moment go out together, and the answers are held for the tab. A
// user the host has no record of is "someone no longer here"; one whose name has not arrived
// yet is plainly "someone", never the id. The names follow the deployment, since an identity's
// name is the one a linked user goes by: a newer deployment reads the held ones again.
import { backoffDelayMs } from "@cairn/client";

import type { Host } from "./host.ts";
import { classify } from "./keyed-read.ts";

/** What a user the host has no record of is called. */
export const GONE = "someone no longer here";
/** What a user is called until the host has answered. */
export const PENDING = "someone";

/** The most users one read names (the API's page limit, PRACTICES). */
const BATCH_MAX = 200;

/** The names held for a tab: asked together, answered together, read again when the deployment moves. */
export class People {
  readonly #host: Host;
  /** A user's name, or null when the host has none. */
  readonly #names = new Map<string, string | null>();
  readonly #wanted = new Set<string>();
  /** Refused (not a passing failure): not asked again until the deployment moves. */
  readonly #failed = new Set<string>();
  /** The read each user was last asked in, so an answer to an older one never replaces a newer. */
  readonly #latest = new Map<string, number>();
  /** Passing failures in a row, for the shared backoff. */
  #attempts = 0;
  #reads = 0;
  readonly #listeners = new Set<() => void>();
  #deployment: number | undefined;
  #version = 0;
  #scheduled = false;

  constructor(host: Host) {
    this.#host = host;
  }

  /** `user`'s name for a screen at `deployment`'s revision, asking the host for it when it is not held. */
  name(user: string, deployment: number | undefined): string {
    if (deployment !== this.#deployment) {
      this.#deployment = deployment;
      this.#failed.clear();
      this.#names.forEach((_, held) => this.#wanted.add(held));
    }
    const held = this.#names.get(user);
    if (held === undefined && !this.#failed.has(user)) {
      this.#wanted.add(user);
    }
    if (this.#wanted.size > 0) {
      this.#schedule();
    }
    return held === undefined ? PENDING : (held ?? GONE);
  }

  readonly subscribe = (listen: () => void): (() => void) => {
    this.#listeners.add(listen);
    return () => {
      this.#listeners.delete(listen);
    };
  };

  /** Changes whenever a name arrives. */
  readonly version = (): number => this.#version;

  #schedule(): void {
    if (this.#scheduled) {
      return;
    }
    this.#scheduled = true;
    queueMicrotask(() => {
      this.#scheduled = false;
      this.#read();
    });
  }

  #read(): void {
    const batch = [...this.#wanted].slice(0, BATCH_MAX);
    batch.forEach((user) => this.#wanted.delete(user));
    if (batch.length === 0) {
      return;
    }
    const read = (this.#reads += 1);
    batch.forEach((user) => this.#latest.set(user, read));
    this.#host.userNames(batch).then(
      (named) => {
        const found = new Map(named.map((each) => [each.user, each.name]));
        this.#attempts = 0;
        batch.filter((user) => this.#latest.get(user) === read).forEach((user) => this.#names.set(user, found.get(user) ?? null));
        this.#version += 1;
        this.#listeners.forEach((listen) => {
          listen();
        });
      },
      (thrown: unknown) => {
        const current = batch.filter((user) => this.#latest.get(user) === read);
        const error = classify(thrown);
        if (error !== "gone" && error.kind === "transient") {
          // A passing failure is read again after the shared backoff (ARCHITECTURE, Reads).
          current.forEach((user) => this.#wanted.add(user));
          setTimeout(() => {
            this.#schedule();
          }, backoffDelayMs(this.#attempts));
          this.#attempts += 1;
        } else {
          current.forEach((user) => this.#failed.add(user));
        }
      },
    );
    if (this.#wanted.size > 0) {
      this.#schedule();
    }
  }
}
