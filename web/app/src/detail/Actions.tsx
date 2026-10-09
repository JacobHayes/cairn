// C11, D1, D4, B2, B5, B6: what a person does to the node (design 6.4, 6.5): its one primary
// action (a decision's is its answer form), at most one secondary beside it, and a `⋯` menu
// listing only what applies. Skip, Snooze, Keep, Include anyway and the guard bypasses open a
// small form in place of the row, never a modal; each is one patch whose rejection shows here.
// A guard that fails disables the primary and says what it waits on; answering or finishing
// anyway is a bypass with a reason (D4). A container holding its own snooze says that moving it
// on lifts the snooze (B6).
import { useState } from "react";
import { useNavigate } from "react-router";

import { assignOwner, missingEvidence, type EvidenceDraft } from "../acting/acts.ts";
import { canvasPath, DEFAULT_VIEW } from "../canvas/settings.ts";
import { mayNotApply } from "../data/activity.ts";
import { deepLinkPath } from "../journeys/address.ts";
import { BreakDown } from "../proposals/Entries.tsx";
import { Menu } from "../screens/Menu.tsx";
import { Button, Field } from "../ui/kit.tsx";
import { AnswerEditor } from "./AnswerEditor.tsx";
import { DoneButton, EvidenceForm, type DoneForm } from "./DoneEvidence.tsx";
import { openSnooze, SnoozePanel, WeightEditor, type SnoozeDraft } from "./editors.tsx";
import { foldKey, setFold } from "./folds.ts";
import { answerable, isBlocked, movesFrom, titleOf, transition, unansweredOf, type Mutation, type NodeDetail, type Ready } from "./model.ts";
import { menuOf, offered, type Act, type MenuId } from "./offers.ts";
import { Rejected } from "./Rejected.tsx";
import { entityName } from "./sections.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "./write.ts";

/**
 * D4: beside a finishing action on undecided work, that finishing it is accepted but may not
 * apply, naming the decisions it waits on; nothing when the node is not undecided.
 */
export function MayNotApply({ view, node }: { view: Ready; node: string }) {
  const unanswered = unansweredOf(view, node);
  if (unanswered.length === 0) {
    return null;
  }
  return (
    <span className="muted small" data-testid="may-not-apply" data-unanswered={unanswered.join(" ")}>
      {mayNotApply(unanswered, (key) => titleOf(view, key))}.
    </span>
  );
}

type ReasonDraft = ReturnType<typeof useFormDraft<string>>;

/** A reason typed and confirmed in place of the action row: the form every "anyway" and skip shares. */
function ReasonForm({ write, form, label, confirm, mutations, testId }: {
  write: NodeWrite;
  form: ReasonDraft;
  label: string;
  confirm: string;
  mutations: (reason: string) => Mutation[];
  testId: string;
}) {
  const reason = form.draft?.value ?? "";
  return (
    <span className="row" data-testid={testId}>
      <Field aria-label={label} placeholder="Why" value={reason} onChange={(event) => { form.change(event.target.value); }} />
      <Button
        primary
        disabled={write.disabled || reason.trim() === ""}
        onClick={() => {
          void write.run(mutations(reason.trim()), form.draft).then((landed) => {
            if (landed) {
              form.close();
            }
          });
        }}
      >
        {confirm}
      </Button>
      <Button onClick={form.close}>Cancel</Button>
    </span>
  );
}

/** D1: skipping a node needs a reason. */
export function SkipForm({ write, node, form }: { write: NodeWrite; node: string; form: ReasonDraft }) {
  return <ReasonForm write={write} form={form} label="Why skip it" confirm="Skip" mutations={(reason) => [transition(node, "skip", reason)]} testId="skip-form" />;
}

/** B6: a transition on a container holding its own snooze clears it, and with it the hold on everything beneath. */
function LiftsSnooze({ view, node, moving }: { view: Ready; node: string; moving: string }) {
  const held = Object.values(view.derived.nodes).filter((derived) => derived.snoozed_via === node).length;
  if (held === 0 || view.journey.graph.state?.snoozes?.[node] === undefined) {
    return null;
  }
  return (
    <span className="muted" data-testid="lifts-snooze">
      {moving} this lifts the snooze on {titleOf(view, node)} and its {held} open {held === 1 ? "item" : "items"}.
    </span>
  );
}

type Guard = "deps_done" | "has_artifact" | "has_note" | "broken_down";

/** The guards a bypass would pass, with what the node is missing in words. */
function guardsOf(view: Ready, detail: NodeDetail): { guards: Guard[]; words: string } {
  const needs = missingEvidence(view, detail.node);
  const missing: [boolean, Guard, string][] = [
    [isBlocked(detail.derived, view), "deps_done", "what it waits on"],
    [needs.artifact, "has_artifact", "a link"],
    [needs.note, "has_note", "a note"],
    [detail.derived.needs_breakdown === true, "broken_down", "a breakdown"],
  ];
  const found = missing.filter(([lacks]) => lacks);
  return { guards: found.map(([, guard]) => guard), words: found.map(([, , words]) => words).join(" and ") };
}

/** F1, D1: Mark reached with the date it was reached on, today unless changed. */
function Reach({ write, node, today, disabled }: { write: NodeWrite; node: string; today: string; disabled: boolean }) {
  const [date, setDate] = useState(today);
  return (
    <span className="row">
      <Button
        primary
        disabled={disabled || write.disabled || date === ""}
        onClick={() => void write.run([transition(node, "reach"), ...(date === today ? [] : [{ op: "set_recorded_date" as const, node, end: "finish" as const, date }])])}
      >
        Mark reached
      </Button>
      <Field type="date" aria-label="Reached on" value={date} onChange={(event) => { setDate(event.target.value); }} />
    </span>
  );
}

/** F2: a reached milestone's actual date, edited. */
function EditDate({ write, node, current, onClose }: { write: NodeWrite; node: string; current: string; onClose: () => void }) {
  const [date, setDate] = useState(current);
  return (
    <span className="row">
      <Field type="date" aria-label="Reached on" value={date} onChange={(event) => { setDate(event.target.value); }} />
      <Button
        primary
        disabled={write.disabled || date === ""}
        onClick={() => {
          void write.run([{ op: "set_recorded_date", node, end: "finish", date }]).then((landed) => {
            if (landed) {
              onClose();
            }
          });
        }}
      >
        Save
      </Button>
      <Button onClick={onClose}>Cancel</Button>
    </span>
  );
}

/** D2: assign the node's owner from the people the deployment lists. */
function AssignOwner({ view, write, node }: { view: Ready; write: NodeWrite; node: string }) {
  const [entity, setEntity] = useState("");
  return (
    <span className="row" data-testid="assign">
      <select aria-label="Assign owner" value={entity} onChange={(event) => { setEntity(event.target.value); }}>
        <option value="">Assign owner</option>
        {(view.inputs.deployment.entities ?? []).map((each) => (
          <option key={each.key} value={each.key}>
            {entityName(view, each.key)}
          </option>
        ))}
      </select>
      <Button disabled={write.disabled || entity === ""} onClick={() => void write.run([assignOwner(node, entity)])}>
        Assign
      </Button>
    </span>
  );
}

/** What a menu entry opens in place of the row. */
type Panel = "assign" | "weight" | "break-down" | "edit-date";

interface Menus {
  view: Ready;
  detail: NodeDetail;
  write: NodeWrite;
  onRename: () => void;
  forms: Record<"skip" | "keep" | "include" | "bypass" | "anyway", ReasonDraft> & { snooze: ReturnType<typeof useFormDraft<SnoozeDraft>> };
  openPanel: (panel: Panel) => void;
  onCopied: () => void;
}

/** What each menu entry does: its one move, or the form it opens. */
function runnersOf({ view, detail, write, onRename, forms, openPanel, onCopied }: Menus, navigate: (to: string) => void): Record<MenuId, () => void> {
  const key = detail.node.key;
  const journey = view.journey.header.id;
  const move = (...mutations: Mutation[]) => () => void write.run(mutations);
  const form = (draft: ReasonDraft) => () => { draft.open("", write.seen); };
  const link = `${globalThis.location.origin}${deepLinkPath(journey, key)}`;
  return {
    snooze: () => { openSnooze(forms.snooze, view, key, write.seen); },
    skip: form(forms.skip),
    stop: move(transition(key, "stop")),
    "done-anyway": form(forms.bypass),
    "reach-anyway": form(forms.bypass),
    "answer-anyway": form(forms.anyway),
    reopen: move(transition(key, "reopen")),
    "edit-date": () => { openPanel("edit-date"); },
    keep: form(forms.keep),
    include: form(forms.include),
    rename: onRename,
    assign: () => { openPanel("assign"); },
    weight: () => { openPanel("weight"); },
    pin: () => { setFold(foldKey(detail.node.kind, "dates"), true); },
    "break-down": () => { openPanel("break-down"); },
    "show-in-graph": () => { navigate(canvasPath(journey, DEFAULT_VIEW, key)); },
    "edit-node": () => { navigate(canvasPath(journey, { ...DEFAULT_VIEW, edit: true }, key)); },
    "copy-link": () => { globalThis.navigator.clipboard.writeText(link).then(onCopied, () => undefined); },
  };
}

/** The `⋯`: each entry does its one thing or opens its form. */
function Overflow(menus: Menus) {
  const navigate = useNavigate();
  const groups = menuOf(menus.view, menus.detail);
  const runners = runnersOf(menus, (to) => void navigate(to));
  if (groups.length === 0) {
    return null;
  }
  return (
    <Menu label="More actions" testId="more-actions" align="end" trigger={<span aria-hidden="true">⋯</span>}>
      {(close) =>
        groups.map((group, at) => (
          <div key={at} className="menu-group">
            {group.map((item) => (
              <button
                key={item.id}
                type="button"
                role="menuitem"
                className="menu-item"
                data-testid={`menu-${item.id}`}
                onClick={() => {
                  close();
                  runners[item.id]();
                }}
              >
                {item.label}
              </button>
            ))}
          </div>
        ))
      }
    </Menu>
  );
}

/** One of the row's buttons. */
function ActButton({ act, view, detail, write, evidence, waiting }: { act: Act; view: Ready; detail: NodeDetail; write: NodeWrite; evidence: DoneForm; waiting: boolean }) {
  const key = detail.node.key;
  const run = (mutations: Mutation[]) => () => void write.run(mutations);
  switch (act) {
    case "done":
      return (
        <Button primary disabled={write.disabled || waiting} onClick={run([transition(key, "complete")])}>
          Mark done
        </Button>
      );
    case "done-with-evidence":
      return <DoneButton write={write} form={evidence} />;
    case "start":
      return (
        <Button disabled={write.disabled} onClick={run([transition(key, "start")])}>
          Start
        </Button>
      );
    case "reach":
      return <Reach write={write} node={key} today={view.derived.today} disabled={waiting} />;
    case "reach-now":
      return (
        <Button primary disabled={write.disabled} onClick={run([transition(key, "reach")])}>
          Confirm reached now
        </Button>
      );
    case "break-down":
      return <BreakDown ready={view} node={detail.node} />;
    case "mark-atomic":
      return (
        <Button disabled={write.disabled} onClick={run([{ op: "set_atomic", node: key, atomic: true }])}>
          Mark atomic
        </Button>
      );
    case "unsnooze":
      return (
        <Button disabled={write.disabled} onClick={run([{ op: "unsnooze", node: key }])}>
          Unsnooze
        </Button>
      );
  }
}

/** The forms and panels a menu entry opened, in place of the row. */
function Opened({ view, detail, write, forms, evidence, panel, onClose }: { view: Ready; detail: NodeDetail; write: NodeWrite; forms: Menus["forms"]; evidence: DoneForm; panel: Panel | undefined; onClose: () => void }) {
  const { node, record } = detail;
  const guards = guardsOf(view, detail);
  const finishing = node.kind === "milestone" ? "reach" : "complete";
  return (
    <>
      {forms.skip.draft === undefined ? null : <SkipForm write={write} node={node.key} form={forms.skip} />}
      {forms.keep.draft === undefined ? null : (
        <ReasonForm write={write} form={forms.keep} label="Why keep it" confirm="Keep" mutations={(reason) => [{ op: "apply_override", node: node.key, override: { keep: { reason } } }]} testId="keep-form" />
      )}
      {forms.include.draft === undefined ? null : (
        <ReasonForm write={write} form={forms.include} label="Why include it" confirm="Include anyway" mutations={(reason) => [{ op: "apply_override", node: node.key, override: { force_include: { reason } } }]} testId="include-form" />
      )}
      {forms.bypass.draft === undefined ? null : (
        <ReasonForm
          write={write}
          form={forms.bypass}
          label="Why bypass the guard"
          confirm={`${node.kind === "milestone" ? "Mark reached" : "Mark done"} without ${guards.words}`}
          mutations={(reason) => [{ op: "apply_override", node: node.key, override: { guard_bypass: { guards: guards.guards, reason } } }, transition(node.key, finishing)]}
          testId="bypass"
        />
      )}
      <EvidenceForm write={write} node={node.key} needs={missingEvidence(view, node)} form={evidence} />
      {forms.snooze.draft === undefined ? null : <SnoozePanel view={view} detail={detail} form={forms.snooze} />}
      {panel === "assign" ? (
        <span className="row">
          <AssignOwner view={view} write={write} node={node.key} />
          <Button onClick={onClose}>Close</Button>
        </span>
      ) : null}
      {panel === "weight" ? (
        <>
          <WeightEditor view={view} detail={detail} />
          <span>
            <Button onClick={onClose}>Close</Button>
          </span>
        </>
      ) : null}
      {panel === "break-down" ? <BreakDown ready={view} node={node} startOpen onCancel={onClose} /> : null}
      {panel === "edit-date" ? <EditDate write={write} node={node.key} current={record.finished_on ?? view.derived.today} onClose={onClose} /> : null}
    </>
  );
}

export function Actions({ view, detail, onRename }: { view: Ready; detail: NodeDetail; onRename: () => void }) {
  const write = useNodeWrite(view, `actions:${detail.node.key}`, detail.node.key);
  const { node, record } = detail;
  const journey = write.journey;
  const forms = {
    skip: useFormDraft<string>(journey, node.key, "skip"),
    keep: useFormDraft<string>(journey, node.key, "keep"),
    include: useFormDraft<string>(journey, node.key, "force-include"),
    bypass: useFormDraft<string>(journey, node.key, "bypass"),
    anyway: useFormDraft<string>(journey, node.key, "answer-anyway"),
    snooze: useFormDraft<SnoozeDraft>(journey, node.key, "snooze"),
  };
  // G2, G4: work that requires a note or a link and has none is finished by Done..., which adds them with the completion.
  const evidence = useFormDraft<EvidenceDraft | string>(journey, node.key, "done-evidence");
  const [panel, setPanel] = useState<Panel | undefined>();
  const [copied, setCopied] = useState(false);
  const offers = offered(view, detail);
  const deciding = answerable(node.kind, record.state);
  const inForm = evidence.draft !== undefined || forms.skip.draft !== undefined || forms.keep.draft !== undefined || forms.include.draft !== undefined || forms.bypass.draft !== undefined || forms.snooze.draft !== undefined;
  const menu = (
    <Overflow
      view={view}
      detail={detail}
      write={write}
      onRename={onRename}
      forms={forms}
      openPanel={(next) => {
        setPanel(next);
        setCopied(false);
      }}
      onCopied={() => {
        setCopied(true);
      }}
    />
  );
  const acts = [...(offers.primary === undefined ? [] : [offers.primary]), ...offers.secondary];
  const moving = movesFrom(node.kind, record.state).includes("start") ? "Starting" : "Finishing";
  return (
    <div className="stack" data-testid="actions">
      {deciding && !inForm && panel === undefined ? <AnswerEditor view={view} detail={detail} menu={menu} anyway={forms.anyway} /> : null}
      {deciding && !inForm && panel === undefined && offers.secondary.includes("unsnooze") ? (
        <span>
          <ActButton act="unsnooze" view={view} detail={detail} write={write} evidence={evidence} waiting={false} />
        </span>
      ) : null}
      {deciding || inForm || panel !== undefined ? null : (
        <>
          {offers.primary === undefined ? null : <LiftsSnooze view={view} node={node.key} moving={moving} />}
          <span className="row actions-row">
            {acts.map((act) => (
              <ActButton key={act} act={act} view={view} detail={detail} write={write} evidence={evidence} waiting={offers.waiting} />
            ))}
            <span className="spacer" />
            {menu}
          </span>
        </>
      )}
      <Opened view={view} detail={detail} write={write} forms={forms} evidence={evidence} panel={panel} onClose={() => { setPanel(undefined); }} />
      {copied ? <span className="muted small">Link copied.</span> : null}
      <Rejected view={view} write={write} />
    </div>
  );
}
