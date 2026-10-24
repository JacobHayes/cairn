// A journey's name, description and owners, edited as one patch (`edit_journey`), the form a draft
// kept across reloads until it is sent (ARCHITECTURE, Web UI).
import type { Ready } from "../detail/model.ts";
import { useFormDraft } from "../detail/write.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { Button, Input } from "../ui/kit.tsx";
import { OwnersField } from "./OwnersField.tsx";

interface HeaderDraft {
  name: string;
  description: string;
  owners: string[];
}

/** Edit mode's name, description and owners (edit mode is offered only where the page offers writes): a button that opens the form, which closes again once saved or cancelled. */
export function HeaderEditor({ ready }: { ready: Ready }) {
  const { header, revision } = ready.journey;
  const seen = { base: revision, deployment: ready.key.deployment_revision };
  const write = useScreenWrite(`journey-header:${header.id}`, header.name);
  const form = useFormDraft<HeaderDraft>(header.id, "journey", "header");
  const start = { name: header.name, description: header.description ?? "", owners: [...(header.owners ?? [])] };
  if (form.draft === undefined) {
    return (
      <span className="row">
        <Button disabled={write.disabled} onClick={() => { form.open(start, seen); }}>
          Edit the name, description and owners
        </Button>
      </span>
    );
  }
  const { base, deployment } = form.draft;
  // A draft kept before owners were edited here has none.
  const stored: Partial<HeaderDraft> = form.draft.value;
  const value: HeaderDraft = { ...form.draft.value, owners: stored.owners ?? [] };
  const save = async () => {
    const description = value.description.trim();
    const mutation = { op: "edit_journey" as const, name: value.name.trim(), ...(description === "" ? {} : { description }), owners: value.owners };
    if (await write.run({ target: { journey: header.id }, baseRevision: base, deploymentRevision: deployment, mutations: [mutation] })) {
      form.close();
    }
  };
  return (
    <div className="stack" data-testid="header-editor">
      <label className="label" htmlFor="header-name">Name</label>
      <Input id="header-name" value={value.name} onChange={(event) => { form.change({ ...value, name: event.target.value }); }} />
      <label className="label" htmlFor="header-description">Description</label>
      <textarea id="header-description" value={value.description} onChange={(event) => { form.change({ ...value, description: event.target.value }); }} />
      <OwnersField label="label" owners={value.owners} onChange={(owners) => { form.change({ ...value, owners }); }} />
      <span className="row">
        <Button primary disabled={write.disabled || value.name.trim() === ""} onClick={() => void save()}>Save</Button>
        <Button onClick={() => { form.close(); write.dismiss(); }}>Cancel</Button>
      </span>
      {write.rejection === undefined ? (
        <WriteStatus view={write.status} />
      ) : (
        <RejectionView rejection={write.rejection} onDismiss={() => { form.open(value, seen); write.dismiss(); }} />
      )}
    </div>
  );
}
