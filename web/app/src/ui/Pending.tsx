// What a Suspense boundary shows while a screen's code is on its way. It counts as the tab's work
// while it is shown, so the sync chip's settled fact waits for the screen that will read.
import type { ReactNode } from "react";

import { useWorking } from "../data/react.ts";

export function Pending({ children }: { children: ReactNode }) {
  useWorking();
  return <p className="muted small">{children}</p>;
}
