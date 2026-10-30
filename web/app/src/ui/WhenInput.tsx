// A date with an optional time of day (A9): a date input and a time input side by side, the
// value the engine stores: a day (`2026-10-10`), or a moment in the deployment's zone with its
// UTC offset (`2026-10-10T08:15+01:00`). Clearing the time leaves a day. A time typed before
// the date is kept and joins the date once it is entered.
import { useState } from "react";

import { momentIn, timeOf } from "../status/when.ts";
import { Input } from "./kit.tsx";

/** The value of `day` with the time `time` (none for a day) in `zone`. */
function joined(zone: string, day: string, time: string): string {
  return day === "" || time === "" ? day : momentIn(zone, day, time);
}

export function WhenInput({ label, value, zone, testid, onChange }: { label: string; value: string; zone: string; testid?: string; onChange: (value: string) => void }) {
  const day = value.slice(0, 10);
  const [waiting, setWaiting] = useState("");
  const time = day === "" ? waiting : (timeOf(value) ?? "");
  return (
    <span className="row when-input">
      <Input
        type="date"
        aria-label={label}
        {...(testid === undefined ? {} : { "data-testid": testid })}
        value={day}
        onChange={(event) => {
          setWaiting(event.target.value === "" ? time : "");
          onChange(joined(zone, event.target.value, time));
        }}
      />
      <Input
        type="time"
        aria-label={`${label}, time of day`}
        {...(testid === undefined ? {} : { "data-testid": `${testid}-time` })}
        value={time}
        onChange={(event) => {
          if (day === "") {
            setWaiting(event.target.value);
          } else {
            onChange(joined(zone, day, event.target.value));
          }
        }}
      />
      <span className="muted small">{zone}</span>
    </span>
  );
}
