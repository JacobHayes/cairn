// The design system's skeleton components (tokens.css): what screens compose, so they share
// one look in light and dark.
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from "react";

import type { Capabilities } from "../data/host.ts";
import { useCapabilities } from "../data/react.ts";

export function Button({
  primary = false,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { primary?: boolean }) {
  const classes = ["button", primary ? "button-primary" : "", className].filter(Boolean).join(" ");
  return <button type="button" className={classes} {...props} />;
}

export function Field(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input className="field" {...props} />;
}

export type Tone = "plain" | "good" | "warn" | "bad";

export function Badge({
  tone = "plain",
  children,
  ...rest
}: {
  tone?: Tone;
  children: ReactNode;
  "data-testid"?: string;
  "data-status"?: string;
}) {
  return (
    <span className={tone === "plain" ? "badge" : `badge badge-${tone}`} {...rest}>
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

/** Capabilities gating: renders `children` only when the host offers what `when` asks. */
export function Gate({ when, children, otherwise = null }: { when: (capabilities: Capabilities) => boolean; children: ReactNode; otherwise?: ReactNode }) {
  return when(useCapabilities()) ? children : otherwise;
}
