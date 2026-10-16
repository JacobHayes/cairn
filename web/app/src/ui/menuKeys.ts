// The arrow-key arithmetic every menu of actions shares (Menu.tsx, ContextMenu.tsx).

/**
 * Which of `count` menu items `key` moves the focus to from item `at` (-1: none focused):
 * ArrowDown and ArrowUp wrap round, Home and End jump to the ends; undefined for any other key,
 * and when there are no items.
 */
export function menuFocusTarget(key: string, at: number, count: number): number | undefined {
  if (count === 0) {
    return undefined;
  }
  switch (key) {
    case "ArrowDown":
      return (at + 1) % count;
    case "ArrowUp":
      return at <= 0 ? count - 1 : at - 1;
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return undefined;
  }
}
