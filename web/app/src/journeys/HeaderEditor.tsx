// A journey's name and description, edited as one patch (`edit_journey`), the form a draft
// kept across reloads until it is sent (ARCHITECTURE, Web UI).
import type { Ready } from "../detail/model.ts";
import { useFormDraft } from "../detail/write.ts";
import { Refused } from "../screens/Refused.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Field } from "../ui/kit.tsx";

interface HeaderDraft {
  name: string;
  description: string;
}

export function HeaderEditor({ ready }: { ready: Ready }) {
  const { header, revision } = ready.journey;
  const seen = { base: revision, deployment: ready.key.deployment_revision };
  const write = useScreenWrite();
  const form = useFormDraft<HeaderDraft>(header.id, "journey", "header");
  const archived = header.status === "archived";
  if (form.draft === undefined) {
    return (
      <span className="row">
        <Button disabled={write.disabled || archived} onClick={() => { form.open({ name: header.name, description: header.description ?? "" }, seen); }}>
          Edit the name and description
        </Button>
      </span>
    );
  }
  const { value, base } = form.draft;
  const save = async () => {
    const description = value.description.trim();
    const mutation = { op: "edit_journey" as const, name: value.name.trim(), ...(description === "" ? {} : { description }) };
    if (await write.run({ target: { journey: header.id }, baseRevision: base, mutations: [mutation] })) {
      form.close();
    }
  };
  return (
    <div className="stack" data-testid="header-editor">
      <Field aria-label="Name" value={value.name} onChange={(event) => { form.change({ ...value, name: event.target.value }); }} />
      <textarea aria-label="Description" value={value.description} onChange={(event) => { form.change({ ...value, description: event.target.value }); }} />
      <span className="row">
        <Button primary disabled={write.disabled || value.name.trim() === ""} onClick={() => void save()}>Save</Button>
        <Button onClick={() => { form.close(); write.dismiss(); }}>Cancel</Button>
      </span>
      {write.rejected === undefined ? null : (
        <Refused rejection={write.rejected} onDismiss={() => { form.open(value, seen); write.dismiss(); }} />
      )}
    </div>
  );
}
