// A10, G3: the node's route-authored resources: markdown tips, typed links, and message
// drafts rendered with the journey's context (its fields, role members, and answers) in the
// derive worker, a placeholder with no value shown as a marker, and copied as text. Sending
// is out of scope. The draft re-renders whenever the journey's derivation moves.
import type { Schema } from "@cairn/client";
import type { RenderedDraft } from "@cairn/wasm";
import { useEffect, useState } from "react";

import { useSession } from "../data/react.ts";
import { Badge, Button } from "../ui/kit.tsx";
import { Markdown, safeHref } from "../ui/markdown.tsx";
import type { NodeDetail, Ready } from "./model.ts";
import { Section } from "./parts.tsx";

type Resource = Schema<"Resource">;

/** G3: the text a rendered draft copies as, each missing placeholder written as a marker. */
export function draftText(draft: RenderedDraft): string {
  return draft.segments.map((segment) => ("text" in segment ? segment.text : `[missing: ${segment.missing}]`)).join("");
}

/** The page's link to the journey (`{{journey.url}}`): its address on this origin. */
export function journeyUrl(journey: string, origin: string): string {
  return `${origin}/journeys/${journey}`;
}

function MessageDraft({ view, node, resource }: { view: Ready; node: string; resource: string }) {
  const { deriver } = useSession();
  const journey = view.journey.header.id;
  const [rendered, setRendered] = useState<RenderedDraft | { failed: string } | undefined>();
  const [copied, setCopied] = useState(false);
  const derivation = `${String(view.key.revision)}:${String(view.key.deployment_revision)}:${view.key.today}`;
  useEffect(() => {
    let live = true;
    deriver.renderDraft(journey, { key: node, resource, url: journeyUrl(journey, location.origin) }).then(
      (draft) => { if (live) setRendered(draft); },
      (thrown: unknown) => { if (live) setRendered({ failed: thrown instanceof Error ? thrown.message : String(thrown) }); },
    );
    return () => {
      live = false;
    };
  }, [deriver, journey, node, resource, derivation]);
  if (rendered === undefined) {
    return <span className="muted">Rendering...</span>;
  }
  if ("failed" in rendered) {
    return <span className="callout callout-bad">The draft could not be rendered: {rendered.failed}</span>;
  }
  const copy = () => {
    void navigator.clipboard.writeText(draftText(rendered)).then(() => { setCopied(true); });
  };
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
        <Button onClick={copy}>Copy</Button>
        {copied ? <span className="muted" data-testid="copied">Copied.</span> : null}
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
    <Section title="Resources" summary={String(resources.length)} open testId="resources">
      <ul className="checklist stack">
        {resources.map((resource) => (
          <Item key={resource.key} view={view} node={detail.node.key} resource={resource} />
        ))}
      </ul>
    </Section>
  );
}
