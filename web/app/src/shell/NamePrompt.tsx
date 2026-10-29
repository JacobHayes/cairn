// H3: "on load, if there's no name, ask". Sign-in made the caller an identity from their verified
// email alone; this modal asks what to call them and saves it as an ordinary identity edit. Later
// closes it for this load; it asks again on the next.
import { useCallback, useEffect, useId, useRef, useState } from "react";

import { useCaller, useDeployment } from "../data/react.ts";
import { CheckFailed, invalidAttrs, Problem } from "../authoring/parts.tsx";
import { useValueProblems } from "../authoring/form-problems.ts";
import { identityOf } from "../people/model.ts";
import { useScreenWrite } from "../screens/write.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Button, Input } from "../ui/kit.tsx";
import { asksName } from "./name-prompt.ts";

/** What the prompt knows, which the shell reads too: the tour waits until the caller and deployment are read and nothing is being asked. */
export interface NameAsk {
  settled: boolean;
  asking: boolean;
  later: () => void;
}

export function useNameAsk(): NameAsk {
  const { caller } = useCaller();
  const deployment = useDeployment();
  const [later, setLater] = useState(false);
  const asking = asksName(caller, deployment) !== undefined && deployment !== undefined && !later;
  const dismiss = useCallback(() => { setLater(true); }, []);
  return { settled: caller !== undefined && deployment !== undefined && !asking, asking, later: dismiss };
}

export function NamePrompt({ ask }: { ask: NameAsk }) {
  const { caller } = useCaller();
  const deployment = useDeployment();
  const [name, setName] = useState("");
  const write = useScreenWrite("name-prompt", "Your name");
  const dialog = useRef<HTMLDialogElement>(null);
  const checks = useValueProblems([{ field: "name", kind: "title", what: "The name", value: name.trim(), optional: true }]);
  const message = useId();
  const identity = asksName(caller, deployment);
  const { asking } = ask;
  useEffect(() => {
    if (asking && dialog.current?.open === false) {
      dialog.current.showModal();
    }
  }, [asking]);
  if (!asking || identity === undefined || deployment === undefined) {
    return null;
  }
  const problem = checks.problems.name;
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
        ask.later();
      }}
    >
      <form className="stack modal-body" onSubmit={(event) => { event.preventDefault(); void save(); }}>
        <strong>What should we call you?</strong>
        <span className="muted">
          This names <span className="mono">{identity.emails?.[0] ?? ""}</span>
          {(identity.emails ?? []).length > 1 ? ` and ${String((identity.emails ?? []).length - 1)} more` : ""}, the identity others see beside your work.
        </span>
        <Input label="Name" {...invalidAttrs(message, problem)} value={name} autoFocus onChange={(event) => { setName(event.target.value); }} />
        {problem === undefined ? null : <Problem id={message}>{problem}</Problem>}
        <CheckFailed failure={checks.failure} retry={checks.retry} />
        {write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />}
        <span className="row">
          <Button primary type="submit" disabled={write.disabled || name.trim() === "" || !checks.checked || problem !== undefined}>Save</Button>
          <Button onClick={ask.later}>Later</Button>
        </span>
      </form>
    </dialog>
  );
}
