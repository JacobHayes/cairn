// The design system's components (DESIGN.md; design/base.css styles the elements and classes
// these render): what screens compose, so they share one look in light and dark. Graticule
// styles `button`, `input`, `select` and `textarea` as elements, so these add a class only for
// a variant (`primary`, `ghost`), and `.field` is its labelled wrapper.
import { useId, type ButtonHTMLAttributes, type HTMLAttributes, type InputHTMLAttributes, type ReactNode } from "react";

import type { Capabilities } from "../data/host.ts";
import { useCapabilities } from "../data/react.ts";
import { Tip, type TipKind } from "./Tip.tsx";

/**
 * A button. `tip` is its tooltip (what pressing it does; `keys` is its key, `tipKind` is `name` for an
 * icon-only one). `held` is a button that cannot act now and says why: it stays focusable
 * (`aria-disabled`), a press does nothing but show the reason, and the reason is its description
 * (a control with no reason to give is plainly `disabled`).
 */
export function Button({
  primary = false,
  ghost = false,
  className = "",
  tip,
  tipKind,
  keys,
  held,
  onClick,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { primary?: boolean; ghost?: boolean; tip?: ReactNode; tipKind?: TipKind; keys?: string; held?: string | undefined }) {
  const classes = [primary ? "primary" : "", ghost ? "ghost" : "", className].filter(Boolean).join(" ");
  const isHeld = held !== undefined;
  const button = (
    <button
      type="button"
      {...(classes === "" ? {} : { className: classes })}
      {...props}
      {...(isHeld ? { "aria-disabled": true, onClick: (event: { preventDefault: () => void }) => { event.preventDefault(); } } : { onClick })}
    />
  );
  if (isHeld) {
    return <Tip tip={held} tap>{button}</Tip>;
  }
  return (
    <Tip tip={tip} {...(tipKind === undefined ? {} : { kind: tipKind })} {...(keys === undefined ? {} : { keys })}>
      {button}
    </Tip>
  );
}

/** A text input: Graticule styles it as an element. With a `label`, it sits under a mono label in a `.field`. */
export function Input({ label, ...props }: InputHTMLAttributes<HTMLInputElement> & { label?: string }) {
  if (label === undefined) {
    return <input {...props} />;
  }
  return (
    <label className="field">
      <span className="label">{label}</span>
      <input {...props} />
    </label>
  );
}

/** The input itself, kept under its old name so code written against it still builds. Use `Input`. */
export const Field = Input;

/**
 * A status dot's tone, by its meaning: `done` done or decided (a check, not a dot), `good` approved or new, `bad` failed, `warn` attention or deferred,
 * `pending` steel; `ink` filled is actionable now and `ring` in progress; `scheduled`, `blocked`
 * (square), `conditional` (dashed), `skipped` (struck) and `superseded` (hollow) keep states
 * apart by shape where colour is close (DESIGN, Status).
 */
export type Tone = "plain" | "done" | "good" | "warn" | "bad" | "pending" | "ink" | "ring" | "scheduled" | "blocked" | "conditional" | "skipped" | "superseded";

export function Badge({
  tone = "plain",
  className,
  children,
  ...rest
}: Omit<HTMLAttributes<HTMLSpanElement>, "title"> & {
  tone?: Tone;
  children: ReactNode;
  "data-testid"?: string;
  "data-status"?: string;
  /** The kind of change a proposal's layer makes to what this chip says (proposals/layer.ts). */
  "data-diff"?: string;
}) {
  return (
    <span className={[tone === "plain" ? "badge" : `badge ${tone}`, className].filter(Boolean).join(" ")} {...rest}>
      {children}
    </span>
  );
}

export function Panel({ children, ...rest }: { children: ReactNode; "aria-label"?: string; "data-testid"?: string }) {
  return (
    <section className="panel stack" {...rest}>
      {children}
    </section>
  );
}

/**
 * A segmented control over native radios, so the arrow keys pick (DESIGN, Segmented control):
 * the choice has an ink underline, and the group is named by `label`.
 */
export function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
  ...rest
}: {
  label: string;
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
  "data-testid"?: string;
}) {
  const name = useId();
  return (
    <div className="segmented" role="radiogroup" aria-label={label} {...rest}>
      {options.map((option) => (
        <label key={option.value}>
          <input
            type="radio"
            name={name}
            value={option.value}
            checked={option.value === value}
            onChange={() => {
              onChange(option.value);
            }}
          />
          {option.label}
        </label>
      ))}
    </div>
  );
}

/**
 * A quiet progress track (DESIGN, Charts): `done` of `of` in graphite on a hairline, as wide as its
 * container, an empty track when nothing is in scope. It is hidden from assistive technology
 * unless a `label` names it, since the figure beside it normally says the same.
 */
export function Meter({ done, of, label, ...rest }: { done: number; of: number; label?: string; "data-testid"?: string; "data-done"?: number }) {
  const share = of === 0 ? 0 : Math.min(done / of, 1) * 100;
  return (
    <span className="meter" {...(label === undefined ? { "aria-hidden": true } : { role: "img", "aria-label": label })} {...rest}>
      <span style={{ width: `${String(share)}%` }} />
    </span>
  );
}

/**
 * What an index shows in place of its table: that it is being read, that reading it failed (with
 * the way to try again, when there is one), or that it holds nothing (with the way to something
 * else). Presentation only: the caller knows which read failed and whether a retry exists.
 */
export function IndexState({ kind, children, action, ...rest }: { kind: "loading" | "failed" | "empty"; children: ReactNode; action?: ReactNode; "data-testid"?: string }) {
  return (
    <div className={`index-state index-state-${kind}`} role={kind === "failed" ? "alert" : "status"} {...rest}>
      <p>{children}</p>
      {action === undefined ? null : <span className="index-state-action">{action}</span>}
    </div>
  );
}

/** Capabilities gating: renders `children` only when the host offers what `when` asks. */
export function Gate({ when, children, otherwise = null }: { when: (capabilities: Capabilities) => boolean; children: ReactNode; otherwise?: ReactNode }) {
  return when(useCapabilities()) ? children : otherwise;
}
