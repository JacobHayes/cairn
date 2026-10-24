// A value as a chip (DESIGN, Multi-value input): a hairline mono label on the band fill, with a
// remove control after it when it can be taken away. Every chip in the app is this one: a
// value in the multi-value input, an identity's emails at rest, a filter that is on. `remove` is
// the control itself (a button, or a link where the removal is an address) named "Remove <value>".
import type { ReactNode } from "react";

export function Chip({ children, remove }: { children: ReactNode; remove?: ReactNode }) {
  return (
    <span className="chip-value">
      <span data-testid="chip-value">{children}</span>
      {remove}
    </span>
  );
}

/** The cross inside a chip's remove control. */
export function Cross() {
  return <span aria-hidden="true">×</span>;
}
