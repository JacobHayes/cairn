// A date with an optional time of day (A9): a date input and a time input side by side, read and
// typed on the viewer's clock. The value is what the engine stores: a day (`2026-10-10`), or the
// same instant as a moment in the deployment's zone with its UTC offset (`2026-10-10T08:15+01:00`).
// Clearing the time leaves a day, clearing the date clears both. A time typed before any date is on
// the viewer's today, so the value always holds what is typed. Both fields carry a label above the control.
import { localWall, momentFrom, viewerToday } from "../status/when.ts";
import { Input } from "./kit.tsx";

/** The value of `day` with the time `time` (none for a day), stored in the deployment's `zone`. */
export function joined(zone: string, day: string, time: string): string {
  return day === "" || time === "" ? day : momentFrom(zone, day, time);
}

/** The value once `time` is typed beside `day`, which is `today` while no date is typed. */
export function timeTyped(zone: string, day: string, time: string, today: string): string {
  return joined(zone, day === "" ? today : day, time);
}

export function WhenInput({ label, value, zone, testid, onChange }: { label: string; value: string; zone: string; testid?: string; onChange: (value: string) => void }) {
  const wall = localWall(value);
  const day = wall?.day ?? value;
  const time = wall?.time ?? "";
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
          onChange(timeTyped(zone, day, event.target.value, viewerToday()));
        }}
      />
    </span>
  );
}
