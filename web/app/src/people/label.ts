// How an identity is shown, wherever it is (H3): the one place that decides.
import { IDENTITY_GONE } from "../ui/gone.ts";

/** H3: an identity's name, else its first email (it has one of them); words for none at all. */
export function identityLabel(identity: { name?: string | null | undefined; emails?: readonly string[] | null | undefined }): string {
  return identity.name ?? identity.emails?.[0] ?? IDENTITY_GONE;
}
