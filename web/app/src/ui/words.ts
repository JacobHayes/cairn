// The two small word helpers every screen shares: a count with its noun, and a list of names.

/** A count with its noun: "1 node", "3 nodes", "2 entries" (`many` when the plural is not `one` + "s"). */
export function plural(count: number, one: string, many = `${one}s`): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

/** Up to this many names are all said; more are cut to one fewer and "and N more". */
const SAID_MAX = 3;

/**
 * A list of names as words, as parts a caller can render: "A", "A and B", "A, B and C", and
 * past three "A, B and 4 more" (never "and 1 more"). Items stay as given; separators are strings.
 * A caller that must not drop a name gives `max` as Infinity.
 */
export function listParts<Item>(items: readonly Item[], max = SAID_MAX): (Item | string)[] {
  const shown = items.length > max ? items.slice(0, max - 1) : items;
  const more = items.length - shown.length;
  const out: (Item | string)[] = [];
  shown.forEach((item, at) => {
    if (at > 0) {
      out.push(at === shown.length - 1 && more === 0 ? " and " : ", ");
    }
    out.push(item);
  });
  if (more > 0) {
    out.push(` and ${String(more)} more`);
  }
  return out;
}

/** A list of names as one phrase (see `listParts`); empty for none. */
export function namedList(names: readonly string[], max = SAID_MAX): string {
  return listParts(names, max).join("");
}
