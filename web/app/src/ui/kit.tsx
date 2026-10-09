// The design system's components (DESIGN.md; design/base.css styles the elements and classes
// these render): what screens compose, so they share one look in light and dark. Graticule
// styles `button`, `input`, `select` and `textarea` as elements, so these add a class only for
// a variant (`primary`, `ghost`), and `.field` is its labelled wrapper.
import { useId, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode } from "react";

import type { Capabilities } from "../data/host.ts";
import { useCapabilities } from "../data/react.ts";

export function Button({
  primary = false,
  ghost = false,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { primary?: boolean; ghost?: boolean }) {
  const classes = [primary ? "primary" : "", ghost ? "ghost" : "", className].filter(Boolean).join(" ");
  return <button type="button" {...(classes === "" ? {} : { className: classes })} {...props} />;
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
 * A status dot's tone, by its meaning: `good` done, `bad` failed, `warn` attention or deferred,
 * `pending` steel; `ink` filled is actionable now and `ring` in progress; `scheduled`, `blocked`
 * (square), `conditional` (dashed), `skipped` (struck) and `superseded` (hollow) keep states
 * apart by shape where colour is close (DESIGN, Status).
 */
export type Tone = "plain" | "good" | "warn" | "bad" | "pending" | "ink" | "ring" | "scheduled" | "blocked" | "conditional" | "skipped" | "superseded";

export function Badge({
  tone = "plain",
  children,
  ...rest
}: {
  tone?: Tone;
  children: ReactNode;
  "data-testid"?: string;
  "data-status"?: string;
  title?: string;
}) {
  return (
    <span className={tone === "plain" ? "badge" : `badge ${tone}`} {...rest}>
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

/** Capabilities gating: renders `children` only when the host offers what `when` asks. */
export function Gate({ when, children, otherwise = null }: { when: (capabilities: Capabilities) => boolean; children: ReactNode; otherwise?: ReactNode }) {
  return when(useCapabilities()) ? children : otherwise;
}
