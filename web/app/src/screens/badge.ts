// A state shown on a toolbar control as a small fixed-size count, with the words that go in the
// control's tooltip, so no control changes width (DESIGN, the view controls).

/** A count on a control, and what it says, for the tooltip. */
export interface Badge {
  count: number;
  words: string;
}
