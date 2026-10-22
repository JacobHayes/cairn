// C16: picking rows on the journey index: a checkbox or a Cmd- or Ctrl-click adds or drops one
// row and becomes the anchor; a Shift-click adds every row from the anchor to the one clicked,
// in the order the table lists them.

/** The ids picked after a Shift-click on `to`: every id from `anchor` to `to` in `order` added to `picked`; just `to` toggled when there is no anchor in the table. */
export function pickRange(order: readonly string[], picked: readonly string[], anchor: string | undefined, to: string): string[] {
  const from = anchor === undefined ? -1 : order.indexOf(anchor);
  const end = order.indexOf(to);
  if (from < 0 || end < 0) {
    return picked.includes(to) ? picked.filter((id) => id !== to) : [...picked, to];
  }
  const span = order.slice(Math.min(from, end), Math.max(from, end) + 1);
  return [...picked, ...span.filter((id) => !picked.includes(id))];
}
