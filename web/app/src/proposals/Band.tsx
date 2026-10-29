// C14: the band, the one line the proposal layer adds under a destination's header: what is
// being reviewed, Apply, and Close. It is the one place a phone shows that the page is not the
// destination as it is. Apply is one click when nothing holds it; while something does it is
// `aria-disabled`, its title says what, and pressing it brings the rail forward and focuses the
// rail's "Apply waits for" line, so the reason is one tap away where a title cannot be read.
import { Link } from "react-router";

import { useFrameActions } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { useLayer } from "./layer.ts";
import { holdsWords } from "./Rail.tsx";
import "./proposals.css";
import { Tip } from "../ui/Tip.tsx";

/** Brings the rail's hold line into view and focuses it, after the column has had a moment to appear. */
function showHolds(): void {
  window.requestAnimationFrame(() => {
    const line = document.querySelector<HTMLElement>('[data-testid="apply-waits"]');
    line?.scrollIntoView({ block: "nearest" });
    line?.focus();
  });
}

export function Band() {
  const layer = useLayer();
  const frame = useFrameActions();
  if (layer === undefined) {
    return null;
  }
  const proposal = layer.review?.proposal;
  const title = proposal?.draft.title;
  const close = (
    <Link className="button" to={layer.exit} data-testid="close-layer">
      Close
    </Link>
  );
  if (layer.mode === "missing" || proposal === undefined) {
    return (
      <div className="layer-band" role="status" data-testid="layer-band" data-mode="missing">
        <span className="layer-band-text">There is no such proposal here.</span>
        <Link to={layer.proposals} data-testid="open-proposals">
          Open proposals
        </Link>
        {close}
      </div>
    );
  }
  if (layer.mode !== "reviewing" || layer.reviewing === undefined) {
    return (
      <div className="layer-band" role="status" data-testid="layer-band" data-mode={layer.mode}>
        <Tip tip={`"${title ?? ""}" was ${layer.mode}`} kind="cut" tap>
          <span className="layer-band-text">
            &quot;{title}&quot; was {layer.mode}
          </span>
        </Tip>
        {close}
      </div>
    );
  }
  const reviewing = layer.reviewing;
  const held = proposal.status !== "open" || reviewing.blockers.length > 0 || reviewing.write.disabled;
  const holds = holdsWords(reviewing.blockers, reviewing.invalid);
  const creates = layer.creates ? `, which creates this ${layer.kind === "route" ? "route" : "journey"}` : "";
  return (
    <div className="layer-band" data-testid="layer-band" data-mode="reviewing" data-revision={proposal.revision}>
      <Tip tip={`Reviewing "${title ?? ""}"${creates}`} kind="cut" tap>
        <span className="layer-band-text">
          Reviewing &quot;{title}&quot;{creates}
        </span>
      </Tip>
      <Button
        primary
        aria-disabled={held ? "true" : undefined}
        tip={holds}
        data-testid="apply-proposal"
        data-blockers={reviewing.blockers.join(" ")}
        onClick={() => {
          if (reviewing.blockers.length > 0) {
            frame?.show("inspector");
            showHolds();
            return;
          }
          if (!held) {
            reviewing.apply();
          }
        }}
      >
        Apply
      </Button>
      {close}
    </div>
  );
}
