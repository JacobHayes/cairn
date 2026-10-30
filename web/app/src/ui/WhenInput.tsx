// A date with an optional time of day (A9): a date input and a time input side by side, the
// value the engine stores: a day (`2026-10-10`), or a moment in the deployment's zone with its
// UTC offset (`2026-10-10T08:15+01:00`). Clearing the time leaves a day, clearing the date
// clears both. A time typed before any date is on `today`, so the value always holds what is
// typed. Both fields carry a label above the control.
import { momentIn, timeOf } from "../status/when.ts";
import { Input } from "./kit.tsx";

/** The value of `day` with the time `time` (none for a day) in `zone`. */
function joined(zone: string, day: string, time: string): string {
  return day === "" || time === "" ? day : momentIn(zone, day, time);
}

export function WhenInput({ label, today, value, zone, testid, onChange }: { label: string; today: string; value: string; zone: string; testid?: string; onChange: (value: string) => void }) {
  const day = value.slice(0, 10);
  const time = timeOf(value) ?? "";
  return (
    <span className="row row-end when-input">
      <Input
        label={label}
        type="date"
        {...(testid === undefined ? {} : { "data-testid": testid })}
        value={day}
        onChange={(event) => {
          onChange(joined(zone, event.target.value, time));
        }}
      />
      <Input
        label="Time"
        type="time"
        aria-label={`${label}, time of day`}
        {...(testid === undefined ? {} : { "data-testid": `${testid}-time` })}
        value={time}
        onChange={(event) => {
          onChange(joined(zone, day === "" ? today : day, event.target.value));
        }}
      />
      <span className="muted small when-zone">{zone}</span>
    </span>
  );
}
