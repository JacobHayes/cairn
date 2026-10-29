// A multi-value input (DESIGN, Multi-value input): each value is a chip with its own remove
// button, and one input takes the next on Enter, a comma, a semicolon, a space, a paste or when
// it loses focus. A value that fails its rules stays in the input under its reason. The list and
// the pending text are the caller's, so a draft keeps both across a reload; `rules` says what
// a value is (chips.ts).
import { useEffect, useId, useRef, useState, type ChangeEvent, type ClipboardEvent, type KeyboardEvent, type RefObject } from "react";

import { Chip, Cross } from "./Chip.tsx";
import { hasSeparator, takeChips, type ChipRules } from "./chips.ts";

interface Chips {
  values: readonly string[];
  /** The text typed and not yet taken. */
  text: string;
  onChange: (values: string[], text: string) => void;
  rules: ChipRules;
}

/** One take at a time (Enter then blur must not start a second), and none applies once the form is gone: a cancelled form's late answer must not recreate its draft. */
function useGate() {
  const busy = useRef(false);
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);
  return {
    enter: () => !busy.current && (busy.current = true),
    leave: () => {
      busy.current = false;
    },
    alive: () => live.current,
  };
}

/** Taking text into the list (its input handlers) and the reason the last take refused something; refused text is selected so the next key replaces it. */
function useTaking({ values, text, onChange, rules }: Chips, input: RefObject<HTMLInputElement | null>) {
  const [problem, setProblem] = useState<string>();
  const [taking, setTaking] = useState(false);
  const gate = useGate();
  // The engine reads each value before it joins the list, so taking answers a moment later; the input is read-only meanwhile, so nothing typed is overwritten by the answer.
  const take = (typed: string) => {
    // What was typed or pasted is the draft's at once (it holds Save, and a reload keeps it); the answer sorts it into chips and what stays.
    if (!gate.enter()) return;
    onChange([...values], typed);
    setTaking(true);
    void takeChips(values, typed, rules)
      .then((taken) => {
        if (!gate.alive()) {
          return;
        }
        setProblem(taken.problem);
        onChange(taken.values, taken.text);
        if (taken.text !== "") {
          requestAnimationFrame(() => {
            if (document.activeElement === input.current) {
              input.current?.select();
            }
          });
        }
      })
      .catch(() => {
        if (gate.alive()) {
          setProblem("The email could not be checked. Try again.");
        }
      })
      .finally(() => {
        gate.leave();
        if (gate.alive()) {
          setTaking(false);
        }
      });
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
      } else if (event.key === "Backspace" && text === "" && values.length > 0 && !taking) {
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
  return { problem, setProblem, handlers, taking };
}

function RemovableChip({ value, onRemove, disabled }: { value: string; onRemove: () => void; disabled: boolean }) {
  return (
    <Chip
      remove={
        <button type="button" aria-label={`Remove ${value}`} disabled={disabled} onClick={onRemove}>
          <Cross />
        </button>
      }
    >
      {value}
    </Chip>
  );
}

export function ChipsInput({
  label,
  placeholder,
  inputMode = "text",
  ...chips
}: Chips & {
  /** The field's name, shown above it and naming the input. */
  label: string;
  placeholder?: string;
  /** The keyboard a phone offers. */
  inputMode?: "text" | "email" | "url";
}) {
  const { values, text, onChange } = chips;
  const input = useRef<HTMLInputElement>(null);
  const reasonId = useId();
  const inputId = useId();
  const { problem, setProblem, handlers, taking } = useTaking(chips, input);
  return (
    <div className="field">
      <label className="label" htmlFor={inputId}>
        {label}
      </label>
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
            disabled={taking}
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
          id={inputId}
          aria-invalid={problem !== undefined}
          aria-describedby={problem === undefined ? undefined : reasonId}
          placeholder={placeholder}
          readOnly={taking}
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
