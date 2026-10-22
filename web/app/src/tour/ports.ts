// The tour's effects over a session: the record through the host, and the practice journey made
// and removed through the edit manager, as every write of the app is, so a receipt lost on the
// way is resent under the same patch id and a committed create is never made twice.
import type { Session } from "../data/session.ts";
import { Missing } from "../data/host.ts";
import { submitWrite, aboutOf } from "../data/edit-manager.ts";
import { idOf, type Answer } from "../data/edit-record.ts";
import { isPatchAnswer, newPatchId } from "../data/writes.ts";
import { newJourneyId } from "../journeys/lifecycle.ts";
import { screenCommand } from "../screens/write.ts";
import type { TourPorts } from "./machine.ts";
import { PRACTICE_NAME, practiceMutations } from "./practice.ts";

/** How many times a removal reads the journey's revision again after one that moved. */
const REMOVE_ATTEMPT_COUNT_MAX = 3;

function landed(answer: Answer | undefined): boolean {
  return answer !== undefined && isPatchAnswer(answer);
}

/** Sends one write through the edit manager; a write the manager keeps as needing the person is dropped, so the tour's own never sits on the sync chip. */
async function sent(session: Session, edit: string, label: string, intent: Parameters<typeof screenCommand>[0]): Promise<boolean> {
  const command = screenCommand(intent, newPatchId());
  const answer = await submitWrite(session.edits, { id: idOf(command), command, about: aboutOf(edit, label, undefined) });
  if (answer === undefined) {
    session.edits.send({ type: "discard", id: idOf(command) });
  }
  return landed(answer);
}

/** The journey's revision now, or none when it does not exist. */
async function revisionOf(session: Session, journey: string): Promise<number | undefined> {
  try {
    const document = JSON.parse(await session.host.documentText(journey)) as { journey: { revision: number } };
    return document.journey.revision;
  } catch (thrown) {
    if (thrown instanceof Missing) {
      return undefined;
    }
    throw thrown;
  }
}

export function tourPorts(session: Session): TourPorts {
  const { host } = session;
  return {
    read: () => host.tour(),
    write: (request) => host.putTour(request),
    newJourneyId: () => newJourneyId(PRACTICE_NAME),
    // A create whose answer never came may still have committed: the journey is looked for before the start is called failed, so a retry never leaves one behind.
    create: async (id, today) =>
      (await sent(session, "tour:create", "The tour's practice journey", { target: { journey: id }, baseRevision: 0, mutations: practiceMutations(today) })) || (await revisionOf(session, id)) !== undefined,
    // A deployment patch naming the journey at the revision read: one that moved makes it stale, and the journey is read again.
    remove: async (id) => {
      for (let attempt = 0; attempt < REMOVE_ATTEMPT_COUNT_MAX; attempt += 1) {
        const revision = await revisionOf(session, id);
        if (revision === undefined) {
          return "gone";
        }
        const deployment = await host.deployment();
        const deleted = await sent(session, "tour:remove", "The tour's practice journey", {
          target: "deployment",
          baseRevision: deployment.revision,
          mutations: [{ op: "delete_journey", journey: id, revision }],
        });
        if (deleted) {
          return "deleted";
        }
      }
      return (await revisionOf(session, id)) === undefined ? "gone" : "failed";
    },
  };
}
