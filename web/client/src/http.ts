// The API's side of the safe retry: one patch sent over HTTP (`POST /{domain}/patches`),
// answered as `retry.ts` reads it. A 409 or 422 carries the rejection unchanged (A15); any
// other refusal is a failure with the server's problem or text.
import type { Client } from "openapi-fetch";

import type { components, paths } from "../generated/api.ts";
import type { Answered, Patch, Rejection, Send } from "./retry.ts";

type Markdown = components["schemas"]["Markdown"];

/** Why a request did not get an answer: the status (0 when none came) and what was said. */
export interface HttpFailure {
  status: number;
  message: string;
}

/** The failure a thrown value (a network error, an abort) stands for. */
export function networkFailure(thrown: unknown): HttpFailure {
  return { status: 0, message: thrown instanceof Error ? thrown.message : String(thrown) };
}

/** The failure an unexpected answer stands for, from its problem body or its text. */
export function answerFailure(status: number, body: unknown): HttpFailure {
  if (typeof body === "object" && body !== null && "message" in body && typeof body.message === "string") {
    return { status, message: body.message };
  }
  return { status, message: typeof body === "string" ? body : `HTTP ${String(status)}` };
}

function isRejection(body: unknown): body is Rejection {
  return typeof body === "object" && body !== null && "rejection" in body;
}

async function post(client: Client<paths>, patch: Patch, note: Markdown | undefined) {
  const body = note === undefined ? { patch } : { patch, note };
  const target = patch.target;
  if (target === "deployment") {
    return client.POST("/deployment/patches", { body });
  }
  if ("journey" in target) {
    return client.POST("/journeys/{id}/patches", { params: { path: { id: target.journey } }, body });
  }
  if ("route" in target) {
    return client.POST("/routes/{id}/patches", { params: { path: { id: target.route } }, body });
  }
  return undefined;
}

/** Sends patches to the server `client` talks to, each with `note` on its events (J1). */
export function sendOver(client: Client<paths>, note?: Markdown): Send<HttpFailure> {
  return async (patch): Promise<Answered<HttpFailure>> => {
    let reply;
    try {
      reply = await post(client, patch, note);
    } catch (thrown) {
      return { outcome: "failed", error: networkFailure(thrown) };
    }
    if (reply === undefined) {
      return { outcome: "failed", error: { status: 0, message: "a proposal is not a domain" } };
    }
    const { data, error, response } = reply;
    if (data !== undefined) {
      return { outcome: "answered", answer: data };
    }
    if ((response.status === 409 || response.status === 422) && isRejection(error)) {
      return { outcome: "rejected", rejection: error };
    }
    return { outcome: "failed", error: answerFailure(response.status, error) };
  };
}
