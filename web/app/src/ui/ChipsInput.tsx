// A multi-value input (DESIGN, Multi-value input): each value is a chip with its own remove
// button, and one input takes the next on Enter, a comma, a semicolon, a space, a paste or when
// it loses focus. A value that fails its rules stays in the input under its reason. The list and
// the pending text are the caller's, so a draft keeps both across a reload; `rules` says what
// a value is (chips.ts).
import { useId, useRef, useState, type ChangeEvent, type ClipboardEvent, type KeyboardEvent, type RefObject } from "react";

import { hasSeparator, takeChips, type ChipRules } from "./chips.ts";

interface Chips {
  values: readonly string[];
  /** The text typed and not yet taken. */
  text: string;
  onChange: (values: string[], text: string) => void;
  rules: ChipRules;
}

/** Taking text into the list (its input handlers) and the reason the last take refused something; refused text is selected so the next key replaces it. */
function useTaking({ values, text, onChange, rules }: Chips, input: RefObject<HTMLInputElement | null>) {
  const [problem, setProblem] = useState<string>();
  const take = (typed: string) => {
    const taken = takeChips(values, typed, rules);
    setProblem(taken.problem);
    onChange(taken.values, taken.text);
    if (taken.text !== "") {
      requestAnimationFrame(() => {
        if (document.activeElement === input.current) {
          input.current?.select();
        }
      });
    }
  };
  const handlers = {
    onChange: (event: ChangeEvent<HTMLInputElement>) => {
      if (hasSeparator(event.target.value)) {
        take(event.target.value);
      } else {
        setProblem(undefined);
        onChange([...values], event.target.value);
      }
    },
    onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter" && text.trim() !== "") {
        event.preventDefault();
        take(text);
      } else if (event.key === "Backspace" && text === "" && values.length > 0) {
        onChange(values.slice(0, -1), "");
        setProblem(undefined);
      }
    },
    onPaste: (event: ClipboardEvent<HTMLInputElement>) => {
      // The pasted text itself, over any selection: a single-line input would flatten its line breaks into nothing.
      event.preventDefault();
      const { selectionStart, selectionEnd } = event.currentTarget;
      take(text.slice(0, selectionStart ?? text.length) + event.clipboardData.getData("text") + text.slice(selectionEnd ?? text.length));
    },
    onBlur: () => {
      if (text.trim() !== "") {
        take(text);
      }
    },
  };
  return { problem, setProblem, handlers };
}

function RemovableChip({ value, onRemove }: { value: string; onRemove: () => void }) {
  return (
    <span className="chip-value">
      <span data-testid="chip-value">{value}</span>
      <button type="button" aria-label={`Remove ${value}`} onClick={onRemove}>
        <span aria-hidden="true">×</span>
      </button>
    </span>
  );
}

export function ChipsInput({
  label,
  placeholder,
  inputMode = "text",
  ...chips
}: Chips & {
  /** The input's accessible name. */
  label: string;
  placeholder?: string;
  /** The keyboard a phone offers. */
  inputMode?: "text" | "email" | "url";
}) {
  const { values, text, onChange } = chips;
  const input = useRef<HTMLInputElement>(null);
  const reasonId = useId();
  const { problem, setProblem, handlers } = useTaking(chips, input);
  return (
    <div className="chips-field">
      <div
        className="chips-input"
        data-invalid={problem === undefined ? undefined : "true"}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            input.current?.focus();
          }
        }}
      >
        {values.map((value) => (
          <RemovableChip
            key={value}
            value={value}
            onRemove={() => {
              onChange(values.filter((held) => held !== value), text);
              setProblem(undefined);
              input.current?.focus();
            }}
          />
        ))}
        <input
          ref={input}
          type="text"
          inputMode={inputMode}
          autoCapitalize="none"
          autoComplete="off"
          spellCheck={false}
          enterKeyHint="done"
          aria-label={label}
          aria-invalid={problem !== undefined}
          aria-describedby={problem === undefined ? undefined : reasonId}
          placeholder={placeholder}
          value={text}
          {...handlers}
        />
      </div>
      {problem === undefined ? null : (
        <span id={reasonId} className="chips-reason" role="alert" data-testid="chips-reason">
          {problem}
        </span>
      )}
    </div>
  );
}
