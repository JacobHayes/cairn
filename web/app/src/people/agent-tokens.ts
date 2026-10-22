// H2: making and revoking agent tokens on the Connect an agent page. These are auth calls, not
// domain writes: no domain, revision or patch id, so the edit manager's rebase, resend and
// persistence do not apply, and resending a mint would make a second secret. One machine owns
// them for as long as the page is open, and a token's secret lives only in it, only while shown.
//
//   mint:   idle -> naming -> minting -> shown        New token; the name; the call (no Cancel once
//                    naming <- minting                it is out); the secret. A failed call keeps
//                                                     the name typed and asks for the list again
//           shown -> idle                            Hide token, or revoking that token, drops the secret
//   revoke: idle -> revoking -> idle                  one row at a time
//
// `listVersion` counts the changes the list must be read again for (a mint, a revoke, a mint
// whose answer was lost); the page reads the list afresh for each.
import { assign, emit, fromPromise, raise, setup, type SnapshotFrom } from "xstate";

import type { AgentTokenHost, MintedToken } from "../data/host.ts";

export interface AgentTokensInput {
  tokens: AgentTokenHost;
}

interface Context {
  tokens: AgentTokenHost;
  name: string;
  /** The secret just made, held only while shown. */
  minted: { name: string; token: MintedToken } | undefined;
  /** What went wrong at the last mint or revoke, in words; cleared by the next attempt. */
  problem: string | undefined;
  /** The token being revoked. */
  revoking: { agent: string; name: string } | undefined;
  listVersion: number;
}

type Event =
  | { type: "open" }
  | { type: "name"; name: string }
  | { type: "make" }
  | { type: "cancel" }
  | { type: "hide" }
  | { type: "revoke"; agent: string; name: string };

/** What the machine announces: a token revoked, for the page to confirm. */
export type AgentTokensEmitted = { type: "revoked"; name: string };

const trimmed = (name: string) => name.trim();

export const agentTokens = setup({
  types: { context: {} as Context, events: {} as Event, emitted: {} as AgentTokensEmitted, input: {} as AgentTokensInput },
  actors: {
    mint: fromPromise<MintedToken, { tokens: AgentTokenHost; name: string }>(({ input }) => input.tokens.mint(input.name)),
    revoke: fromPromise<undefined, { tokens: AgentTokenHost; agent: string }>(async ({ input }) => {
      await input.tokens.revoke(input.agent);
      return undefined;
    }),
  },
  actions: {
    changed: assign({ listVersion: ({ context }) => context.listVersion + 1 }),
    revoked: emit(({ context }) => ({ type: "revoked" as const, name: context.revoking?.name ?? "" })),
  },
  guards: {
    named: ({ context }) => trimmed(context.name) !== "",
    revokesShown: ({ context }) => context.revoking !== undefined && context.minted?.token.agent === context.revoking.agent,
  },
}).createMachine({
  id: "agentTokens",
  type: "parallel",
  context: ({ input }) => ({ tokens: input.tokens, name: "", minted: undefined, problem: undefined, revoking: undefined, listVersion: 0 }),
  states: {
    mint: {
      initial: "idle",
      states: {
        idle: { on: { open: { target: "naming", actions: assign({ name: "", problem: undefined }) } } },
        naming: {
          on: {
            name: { actions: assign({ name: ({ event }) => event.name }) },
            make: { guard: "named", target: "minting", actions: assign({ problem: undefined }) },
            cancel: { target: "idle", actions: assign({ name: "", problem: undefined }) },
          },
        },
        minting: {
          invoke: {
            src: "mint",
            input: ({ context }) => ({ tokens: context.tokens, name: trimmed(context.name) }),
            onDone: { target: "shown", actions: [assign({ minted: ({ context, event }) => ({ name: trimmed(context.name), token: event.output }), name: "" }), "changed"] },
            // No answer: the token may exist, so the list is read again before another is made.
            onError: { target: "naming", actions: [assign({ problem: "The token could not be made, or the answer was lost. Check the list before making another." }), "changed"] },
          },
        },
        shown: { on: { hide: { target: "idle", actions: assign({ minted: undefined }) } } },
      },
    },
    revoke: {
      initial: "idle",
      states: {
        idle: { on: { revoke: { target: "revoking", actions: assign({ revoking: ({ event }) => ({ agent: event.agent, name: event.name }), problem: undefined }) } } },
        revoking: {
          invoke: {
            src: "revoke",
            input: ({ context }) => ({ tokens: context.tokens, agent: context.revoking?.agent ?? "" }),
            onDone: [
              // The token on show is revoked: its commands would no longer work, so its secret goes.
              { guard: "revokesShown", target: "idle", actions: ["revoked", raise({ type: "hide" }), assign({ revoking: undefined }), "changed"] },
              { target: "idle", actions: ["revoked", assign({ revoking: undefined }), "changed"] },
            ],
            onError: { target: "idle", actions: [assign({ revoking: undefined, problem: "That token could not be revoked. Try again." }), "changed"] },
          },
        },
      },
    },
  },
});

export type AgentTokensSnapshot = SnapshotFrom<typeof agentTokens>;
