// H3: "on load, if there's no name, ask". Sign-in made the caller an identity from their verified
// email alone; this modal asks what to call them and saves it as an ordinary identity edit. Later
// closes it for this load; it asks again on the next.
import { useEffect, useRef, useState } from "react";

import { useCaller, useDeployment } from "../data/react.ts";
import { identityOf } from "../people/model.ts";
import { useScreenWrite } from "../screens/write.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { Button, Field } from "../ui/kit.tsx";
import { asksName } from "./name-prompt.ts";

export function NamePrompt() {
  const { caller } = useCaller();
  const deployment = useDeployment();
  const [later, setLater] = useState(false);
  const [name, setName] = useState("");
  const write = useScreenWrite("name-prompt", "Your name");
  const dialog = useRef<HTMLDialogElement>(null);
  const identity = asksName(caller, deployment);
  const asking = identity !== undefined && deployment !== undefined && !later;
  useEffect(() => {
    if (asking && dialog.current?.open === false) {
      dialog.current.showModal();
    }
  }, [asking]);
  if (!asking) {
    return null;
  }
  const save = async () => {
    const mutation = { op: "edit_identity" as const, identity: identityOf(identity.key, name, identity.emails ?? []) };
    await write.run({ target: "deployment", baseRevision: deployment.revision, mutations: [mutation] });
  };
  return (
    <dialog
      ref={dialog}
      className="modal panel"
      aria-label="Your name"
      data-testid="name-prompt"
      onCancel={(event) => {
        event.preventDefault();
        setLater(true);
      }}
    >
      <form className="stack modal-body" onSubmit={(event) => { event.preventDefault(); void save(); }}>
        <strong>What should we call you?</strong>
        <span className="muted">Cairn knows you by {(identity.emails ?? []).join(", ")}. A name is what others see beside your work.</span>
        <Field label="Name" value={name} autoFocus onChange={(event) => { setName(event.target.value); }} />
        {write.rejection === undefined ? null : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />}
        <span className="row">
          <Button primary type="submit" disabled={write.disabled || name.trim() === ""}>Save</Button>
          <Button onClick={() => { setLater(true); }}>Later</Button>
        </span>
      </form>
    </dialog>
  );
}
