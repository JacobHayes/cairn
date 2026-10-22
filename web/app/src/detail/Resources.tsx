// A10, G3: the node's route-authored resources: markdown tips, typed links, and message
// drafts rendered with the journey's context (its fields, role members, and answers) in the
// derive worker, a placeholder with no value shown as a marker, and copied as text. Sending
// is out of scope. The draft re-renders whenever the journey's derivation moves.
import type { Schema } from "@cairn/client";
import type { RenderedDraft } from "@cairn/wasm";

import type { Deriver } from "../data/host.ts";
import { PROJECTION, valueOf, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import { Badge } from "../ui/kit.tsx";
import { CopyButton } from "../ui/CopyButton.tsx";
import { Markdown, safeHref } from "../ui/markdown.tsx";
import { foldKey } from "./folds.ts";
import { journeyPath } from "../journeys/address.ts";
import type { NodeDetail, Ready } from "./model.ts";
import { Section } from "./parts.tsx";
import { Failed } from "../ui/Failed.tsx";

type Resource = Schema<"Resource">;

/** G3: the text a rendered draft copies as, each missing placeholder written as a marker. */
export function draftText(draft: RenderedDraft): string {
  return draft.segments.map((segment) => ("text" in segment ? segment.text : `[missing: ${segment.missing}]`)).join("");
}

/** The page's link to the journey (`{{journey.url}}`): its address on this origin (the journey, which opens on its Summary). */
export function journeyUrl(journey: string, origin: string): string {
  return `${origin}${journeyPath({ journey }, "summary")}`;
}

/** A message draft rendered in the derive worker. */
const DRAFT: ReadSource<{ deriver: Deriver; journey: string; node: string; resource: string }, RenderedDraft> = {
  policy: PROJECTION,
  read: ({ deriver, journey, node, resource }) => deriver.renderDraft(journey, { key: node, resource, url: journeyUrl(journey, location.origin) }),
};

function MessageDraft({ view, node, resource }: { view: Ready; node: string; resource: string }) {
  const { deriver } = useSession();
  const journey = view.journey.header.id;
  const read = useKeyedRead(DRAFT, { of: [journey, node, resource], freshness: [JSON.stringify(view.key)], args: { deriver, journey, node, resource } }).view;
  const rendered = valueOf(read);
  if (rendered === undefined) {
    return read.status === "failed" ? <span className="callout callout-bad"><Failed what="The draft" verb="rendered" error={read.error} /></span> : <span className="muted small">Rendering...</span>;
  }
  return (
    <div className="stack" data-testid="message-draft">
      <blockquote className="draft" data-testid="draft-text">
        {rendered.segments.map((segment, at) =>
          "text" in segment ? (
            <span key={at}>{segment.text}</span>
          ) : (
            <mark key={at} data-testid="draft-missing" title="Nothing fills this yet">
              [missing: {segment.missing}]
            </mark>
          ),
        )}
      </blockquote>
      <span className="row">
        <CopyButton what="the message" text={draftText(rendered)} />
      </span>
    </div>
  );
}

function Item({ view, node, resource }: { view: Ready; node: string; resource: Resource }) {
  const kind = resource.tip !== undefined ? "tip" : resource.message_draft !== undefined ? "message draft" : resource.template !== undefined ? "template" : resource.example !== undefined ? "example" : "reference";
  const link = resource.template ?? resource.example ?? resource.reference;
  return (
    <li className="stack attachment" data-testid="resource" data-key={resource.key}>
      <span className="row">
        <Badge>{kind}</Badge>
        {resource.title == null ? null : <strong>{resource.title}</strong>}
        {link === undefined ? null : safeHref(link) ? <a href={link} target="_blank" rel="noreferrer noopener">{link}</a> : <span className="mono">{link}</span>}
      </span>
      {resource.tip === undefined ? null : <Markdown text={resource.tip} />}
      {resource.message_draft === undefined ? null : <MessageDraft view={view} node={node} resource={resource.key} />}
    </li>
  );
}

/** A10: the route's guidance on this node. */
export function ResourceList({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const resources = detail.node.resources ?? [];
  if (resources.length === 0) {
    return null;
  }
  return (
    <Section title="Guidance" summary={String(resources.length)} open={detail.derived.display_state === "ready" || detail.derived.display_state === "active"} fold={foldKey(detail.node.kind, "guidance")} testId="resources">
      <ul className="plain-list stack">
        {resources.map((resource) => (
          <Item key={resource.key} view={view} node={detail.node.key} resource={resource} />
        ))}
      </ul>
    </Section>
  );
}
