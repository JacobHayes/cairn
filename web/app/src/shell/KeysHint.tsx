// A quiet "?" button on the rail's "You" line, drawn only while the page on screen binds keys (a
// journey or a route). A press opens that page's key sheet, as the `?` key does; hovering or
// focusing it previews the same table in the shared tooltip, drawn as a popover. The preview is
// hidden from assistive technology: the sheet is the accessible form of the keys.
import { KeyTable } from "../screens/KeySheet.tsx";
import { Tip } from "../ui/Tip.tsx";
import { useFrameState } from "./frame.tsx";

export function KeysHint() {
  const keys = useFrameState()?.keys;
  if (keys === undefined) {
    return null;
  }
  return (
    <Tip
      look="panel"
      kind="name"
      tip={
        <>
          <div className="popover-head">Keys</div>
          <div className="popover-body">
            <KeyTable bindings={keys.bindings} />
          </div>
        </>
      }
    >
      <button type="button" className="ghost keys-hint-button" aria-label="Keyboard shortcuts" aria-keyshortcuts="?" onClick={keys.open}>
        ?
      </button>
    </Tip>
  );
}
