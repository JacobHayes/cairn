// Identity and references: what a new node, role, kind, or resource is called. Its key is
// minted once in the tab, with its type's prefix and random, so it never collides with a key
// the graph holds or retired; its id is a slug from its title, unique among its siblings.
import { ID_BYTES_MAX } from "./limits.ts";

/** The key prefixes (crates/schema `id.rs`). */
export type KeyPrefix = "n_" | "r_" | "k_" | "a_" | "i_";

/** Random key bodies: twelve base-36 digits from the browser's random UUID. */
function randomBody(): string {
  return BigInt(`0x${crypto.randomUUID().replaceAll("-", "")}`)
    .toString(36)
    .slice(0, 12);
}

/** A fresh key with `prefix`. */
export function mintKey(prefix: KeyPrefix, random: () => string = randomBody): string {
  return `${prefix}${random()}`;
}

/** The id slug pattern (crates/schema `id.rs`): lowercase letters, digits, `_`, and `-`, led by a letter or digit. */
export const SLUG = /^[a-z0-9][a-z0-9_-]*$/;

/** Whether `text` is an id slug within the id limit. */
export function isSlug(text: string): boolean {
  return SLUG.test(text) && text.length <= ID_BYTES_MAX;
}

/** A title as a slug: lowercase ASCII words joined by `-`, or `node` when nothing is left. */
export function slugOf(title: string, fallback = "node"): string {
  const slug = title
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, ID_BYTES_MAX - 4)
    .replace(/-+$/g, "");
  return slug === "" ? fallback : slug;
}

/** `base`, or `base-2`, `base-3`, ... : the first that none of `taken` holds (Identity: sibling-unique). */
export function uniqueId(base: string, taken: ReadonlySet<string>): string {
  if (!taken.has(base)) {
    return base;
  }
  for (let suffix = 2; suffix <= taken.size + 1; suffix += 1) {
    const candidate = `${base}-${String(suffix)}`;
    if (!taken.has(candidate)) {
      return candidate;
    }
  }
  return `${base}-${String(taken.size + 2)}`;
}
