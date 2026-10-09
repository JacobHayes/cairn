// B11: whether completing a journey is suggested, from the engine's status summary and the
// final milestone, kept current as the journey is derived again.
import { useEffect, useState } from "react";

import { useSession } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { completionSuggested } from "./lifecycle.ts";

export function useSuggested(ready: Ready): boolean {
  const { deriver } = useSession();
  const [suggested, setSuggested] = useState(false);
  useEffect(() => {
    let live = true;
    deriver.project(ready.journey.header.id, { projection: "status_summary" }).then(
      (summary) => {
        if (live) {
          setSuggested(completionSuggested(ready.journey.graph, ready.derived, summary));
        }
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [deriver, ready]);
  return suggested;
}
