// Words a formatter said (status/when.ts): the dates and times of day in them in mono, the rest as it is.
import { dateParts } from "../status/when.ts";

export function Words({ text }: { text: string }) {
  return (
    <>
      {dateParts(text).map((part, at) => (part.date ? <span key={at} className="mono">{part.text}</span> : part.text))}
    </>
  );
}
