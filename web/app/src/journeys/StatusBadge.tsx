// A journey's status as a chip, the same wherever a journey is named (its header, the index).
import { Badge } from "../ui/kit.tsx";
import type { JourneyStatus } from "./address.ts";

/** The status as the Status row and its filter say it. */
export const STATUS_WORDS: Record<JourneyStatus, string> = { active: "Active", completed: "Completed", archived: "Archived" };

/** What a status other than active means, said on its chip. */
const STATUS_MEANING: Record<Exclude<JourneyStatus, "active">, string> = {
  completed: "Completed: it has left mine and the cross-journey lists. Reopen it under the menu, Status.",
  archived: "Archived: it accepts only un-archiving or deletion. Both are under the menu, Status.",
};

/** B11: the status beside a journey's name, read-only, and only when it is not active, unless `always` (a list where the statuses are compared). */
export function JourneyStatusBadge({ status, changed = false, always = false }: { status: JourneyStatus; changed?: boolean; always?: boolean }) {
  return status === "active" && !changed && !always ? null : (
    <Badge data-status={status} {...(changed ? { "data-diff": "changed" } : {})} data-testid="journey-status" title={status === "active" ? STATUS_WORDS.active : STATUS_MEANING[status]}>
      {STATUS_WORDS[status]}
    </Badge>
  );
}
