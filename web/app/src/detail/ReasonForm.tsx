// The reason form every "anyway" and skip shares, and the Cancel button that ends a form's row:
// kept apart from the panel so a refusal's retry can use the same form.
import { useEffect, useRef } from "react";

import { Button, Input } from "../ui/kit.tsx";
import { Saving, type Sending } from "../ui/Receipt.tsx";

/** The end of a form's row of buttons: Cancel, or while its write is out Close (closing leaves the save going). */
export function Dismiss({ sending, onClose }: { sending: Sending; onClose: () => void }) {
  return <Button onClick={onClose}>{sending ? "Close" : "Cancel"}</Button>;
}

/** A reason typed and confirmed in place of the action row: the form every "anyway" and skip shares. Going ahead anyway takes an optional one. */
export function ReasonForm({ value, label, hint, optional = false, confirm, testId, sending, disabled, onChange, onConfirm, onClose }: {
  value: string;
  label: string;
  hint?: string;
  /** The reason may be left empty: the confirming button is enabled with the field blank. */
  optional?: boolean;
  confirm: string;
  testId: string;
  sending: Sending;
  disabled: boolean;
  onChange: (reason: string) => void;
  onConfirm: (reason: string) => void;
  onClose: () => void;
}) {
  // A form opened low in a short sheet (a phone with its keyboard up) is brought wholly into view: the field, its hint and the confirming button.
  const shown = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    shown.current?.scrollIntoView({ block: "nearest" });
  }, []);
  return (
    <span className="stack reason-form" data-testid={testId} ref={shown}>
      <label className="stack">
        <span className="label">{label}</span>
        <Input value={value} onChange={(event) => { onChange(event.target.value); }} />
      </label>
      {hint === undefined ? null : <p className="muted">{hint}</p>}
      <span className="row">
        <Button primary disabled={disabled || (!optional && value.trim() === "")} onClick={() => { onConfirm(value.trim()); }}>
          {confirm}
        </Button>
        <Dismiss sending={sending} onClose={onClose} />
      </span>
      <Saving sending={sending} />
    </span>
  );
}
