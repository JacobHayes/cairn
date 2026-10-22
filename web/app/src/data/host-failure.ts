// The in-browser host's failures, told as the server would tell them (H5): the status the API
// answers for the same refusal, so a write that was refused is not mistaken for one that got no
// answer. The write machine stops a 4xx at once and resends a status 0 with backoff.
import type { HttpFailure } from "@cairn/client";
import { HostFailure, type HostError } from "@cairn/wasm";

/**
 * The status the API answers for each way the browser host refuses a call (ARCHITECTURE, HTTP
 * API): a request it cannot read is a 400, one it reads but will not do a 422 (a draft Cairn
 * cannot make included), a subject it does not hold a 404, and its own breakage a 500, which
 * resending does not mend. A read that raced a newer document (`superseded`) is 0: trying again
 * is right. A `rejected` call is answered as a rejection before this is asked.
 */
const STATUS: Record<HostError["error"], number> = {
  unreadable: 400,
  invalid: 422,
  refused: 422,
  rejected: 422,
  missing: 404,
  failed: 500,
  aborted: 500,
  superseded: 0,
};

/** The failure a thrown value stands for: a `HostFailure` by its reason, anything else as no answer. */
export function hostFailure(thrown: unknown): HttpFailure {
  if (thrown instanceof HostFailure) {
    const { reason } = thrown;
    return { status: STATUS[reason.error], message: "message" in reason ? reason.message : reason.error };
  }
  return { status: 0, message: thrown instanceof Error ? thrown.message : String(thrown) };
}
