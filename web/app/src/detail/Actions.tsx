// C11, D1, D4, B2, B5, B6: what a person does to the node (design 6.4, 6.5): its one primary
// action (a decision's is its answer form), at most one secondary beside it, and a `⋯` menu
// listing only what applies. Skip, Snooze, Keep, Include anyway and the guard bypasses open a
// small form in place of the row, never a modal; each is one patch whose rejection shows here.
// At most one form is open at a time, owned by one machine (actions-form.ts) and kept as one
// draft; a form that sent its write stays until that write ends, and text typed since is never
// lost to the landing. A guard that fails disables the primary and says what it waits on;
// answering or finishing anyway is a bypass with a reason (D4). A container holding its own
// snooze says that moving it on lifts the snooze (B6).
import { useActorRef, useSelector } from "@xstate/react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useLocation, useNavigate } from "react-router";

import { useFrameActions } from "../shell/frame.tsx";
import { assignOwner, doneMutations, evidenceDraft, missingEvidence } from "../acting/acts.ts";
import { graphPath, graphViewFrom } from "../canvas/settings.ts";
import { mayNotApply } from "../data/activity.ts";
import { writeDraft } from "../data/drafts.ts";
import type { OperationId } from "../data/edit-record.ts";
import { useSession } from "../data/react.ts";
import { newPatchId } from "../data/writes.ts";
import { journeyPath } from "../journeys/address.ts";
import { BreakDown } from "../proposals/Entries.tsx";
import { Menu } from "../screens/Menu.tsx";
import { useInsertEntry } from "../segments/entry.ts";
import { insertionRoot, memberOrigin } from "../segments/model.ts";
import { Button, Field } from "../ui/kit.tsx";
import { actionsDraftKey, actionsForm, changing, formView, opening, takeOpened, type Drafted, type FormCase, type FormValues, type FormView, type Opened, type PanelCase } from "./actions-form.ts";
import { AnswerEditor, type Anyway } from "./AnswerEditor.tsx";
import { DoneButton, EvidenceForm } from "./DoneEvidence.tsx";
import { SnoozePanel, snoozeWords, WeightEditor } from "./editors.tsx";
import { foldKey, setFold } from "./folds.ts";
import { answerable, isBlocked, movesFrom, titleOf, transition, unansweredOf, type GraphNode, type Mutation, type NodeDetail, type Ready } from "./model.ts";
import { menuOf, offered, type Act, type ContextId, type MenuId, type MenuItem } from "./offers.ts";
import { Rejected } from "./Rejected.tsx";
import { entityName } from "./sections.tsx";
import { snoozeStart } from "./snooze.ts";
import { Saving, type Sending } from "../ui/Receipt.tsx";
import { guardWords, keptWords } from "./words.ts";
import { useNodeWrite, type NodeWrite } from "./write.ts";

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

/** The end of a form's row of buttons: Cancel, or while its write is out Close (closing leaves the save going). */
function Dismiss({ sending, onClose }: { sending: Sending; onClose: () => void }) {
  return <Button onClick={onClose}>{sending ? "Close" : "Cancel"}</Button>;
}

/** A reason typed and confirmed in place of the action row: the form every "anyway" and skip shares. */
function ReasonForm({ value, label, confirm, testId, sending, disabled, onChange, onConfirm, onClose }: {
  value: string;
  label: string;
  confirm: string;
  testId: string;
  sending: Sending;
  disabled: boolean;
  onChange: (reason: string) => void;
  onConfirm: (reason: string) => void;
  onClose: () => void;
}) {
  return (
    <span className="stack" data-testid={testId}>
      <span className="row">
        <Field aria-label={label} placeholder="Why" value={value} onChange={(event) => { onChange(event.target.value); }} />
        <Button primary disabled={disabled || value.trim() === ""} onClick={() => { onConfirm(value.trim()); }}>
          {confirm}
        </Button>
        <Dismiss sending={sending} onClose={onClose} />
      </span>
      <Saving sending={sending} />
    </span>
  );
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
  const missing: [boolean, Guard][] = [
    [isBlocked(detail.derived, view), "deps_done"],
    [needs.artifact, "has_artifact"],
    [needs.note, "has_note"],
    [detail.derived.needs_breakdown === true, "broken_down"],
  ];
  const found = missing.filter(([lacks]) => lacks).map(([, guard]) => guard);
  return { guards: found, words: found.map(guardWords).join(" and ") };
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
function EditDate({ value, sending, disabled, onChange, onSave, onClose }: { value: string; sending: Sending; disabled: boolean; onChange: (date: string) => void; onSave: (date: string) => void; onClose: () => void }) {
  return (
    <span className="stack" data-testid="edit-date-form">
      <span className="row">
        <Field type="date" aria-label="Reached on" value={value} onChange={(event) => { onChange(event.target.value); }} />
        <Button primary disabled={disabled || value === ""} onClick={() => { onSave(value); }}>
          Save
        </Button>
        <Dismiss sending={sending} onClose={onClose} />
      </span>
      <Saving sending={sending} />
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

/** What the node's actions have open: the machine's view, and the ways to change it. */
interface ActionsForm {
  view: FormView;
  /** What is open in place of the action row (the answer form's reason is not), or none. */
  open: Opened | undefined;
  openForm: <K extends FormCase>(kind: K, value: FormValues[K]) => void;
  openPanel: (panel: PanelCase) => void;
  change: <K extends FormCase>(kind: K, value: FormValues[K]) => void;
  /** The form sent the write `id`, built from the draft at `version`. */
  sent: (id: OperationId, version: number) => void;
  /** A retry sent the write `id` in the place of the sent write `from`. */
  resent: (from: OperationId, id: OperationId) => void;
  close: () => void;
}

/**
 * The node's one open form (actions-form.ts): restored from its draft when the node is shown,
 * saved on each change. `seen` is what a form opened now is drafted against.
 */
function useActionsForm(journey: string, node: string, seen: NodeWrite["seen"], onOpened: () => void): ActionsForm {
  const { edits } = useSession();
  const key = actionsDraftKey(journey, node);
  const [restored] = useState(() => takeOpened(key));
  const actor = useActorRef(actionsForm, {
    input: {
      edits,
      restored,
      save: (opened) => {
        writeDraft(key, opened);
      },
    },
  });
  const snapshot = useSelector(actor, (each) => each);
  const view = useMemo(() => formView(snapshot), [snapshot]);
  const open = (opened: Opened) => {
    onOpened();
    actor.send({ type: "open", opened });
  };
  return {
    view,
    open: view.state === "closed" ? undefined : view.opened,
    openForm: (kind, value) => { open(opening(kind, value, seen)); },
    openPanel: (panel) => { open({ case: panel }); },
    change: (kind, value) => {
      actor.send(changing(kind, value));
    },
    sent: (id, version) => { actor.send({ type: "sent", id, version }); },
    resent: (from, id) => { actor.send({ type: "resent", from, id }); },
    close: () => { actor.send({ type: "close" }); },
  };
}

interface Menus {
  view: Ready;
  detail: NodeDetail;
  write: NodeWrite;
  form: ActionsForm;
  onRename: () => void;
  onCopied: () => void;
  /** Starts inserting a segment under a container; none where the screen has no stepper. */
  insertHere: ((parent: string) => void) | undefined;
}

/** What each menu entry does: its one move, or the form it opens. */
function runnersOf({ view, detail, write, form, onRename, onCopied, insertHere }: Menus, navigate: (to: string, state?: unknown) => void, search: string): Record<MenuId, () => void> {
  const key = detail.node.key;
  const origin = memberOrigin(view.journey.graph, key);
  const journey = view.journey.header.id;
  const move = (...mutations: Mutation[]) => () => void write.run(mutations);
  const reason = (kind: "skip" | "keep" | "include" | "bypass" | "anyway") => () => { form.openForm(kind, ""); };
  // The view the person is on goes with them (the graph draws the node whatever it hides); the copied link is the node's own address.
  const graph = graphViewFrom(new URLSearchParams(search));
  const link = `${globalThis.location.origin}${journeyPath({ journey, node: key }, "graph")}`;
  return {
    snooze: () => { form.openForm("snooze", snoozeStart(view, key)); },
    skip: reason("skip"),
    stop: move(transition(key, "stop")),
    "done-anyway": reason("bypass"),
    "reach-anyway": reason("bypass"),
    "answer-anyway": reason("anyway"),
    reopen: move(transition(key, "reopen")),
    "edit-date": () => { form.openForm("edit-date", detail.record.finished_on ?? view.derived.today); },
    keep: reason("keep"),
    include: reason("include"),
    rename: onRename,
    assign: () => { form.openPanel("assign"); },
    weight: () => { form.openPanel("weight"); },
    pin: () => { setFold(foldKey(detail.node.kind, "dates"), true); },
    "break-down": () => { form.openPanel("break-down"); },
    "show-in-graph": () => { navigate(graphPath(journey, graph, { node: key }, search)); },
    "insert-segment": () => { insertHere?.(key); },
    "show-insertion": () => {
      const root = (origin === undefined ? undefined : insertionRoot(view.journey.graph, origin.insertion)) ?? key;
      // The graph draws the insertion's root and its containers whatever the view hides.
      navigate(graphPath(journey, graph, { node: root }, search), { reveal: { key: root, how: "centre" } });
    },
    "edit-node": () => { navigate(graphPath(journey, { ...graph, edit: true }, { node: key }, search)); },
    "copy-link": () => { globalThis.navigator.clipboard.writeText(link).then(onCopied, () => undefined); },
  };
}

/** The `⋯`: each entry does its one thing or opens its form. */
function Overflow({ groups, runners }: { groups: MenuItem[][]; runners: Record<MenuId, () => void> }) {
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

/** What a row action sends as its one move, if it is one (the others open a form). */
function actMutation(act: Act, key: string): Mutation | undefined {
  switch (act) {
    case "done":
      return transition(key, "complete");
    case "start":
      return transition(key, "start");
    case "reach-now":
      return transition(key, "reach");
    case "mark-atomic":
      return { op: "set_atomic", node: key, atomic: true };
    case "unsnooze":
      return { op: "unsnooze", node: key };
    case "done-with-evidence":
    case "reach":
    case "break-down":
      return undefined;
  }
}

/** One of the row's buttons. */
function ActButton({ act, view, detail, write, form, waiting }: { act: Act; view: Ready; detail: NodeDetail; write: NodeWrite; form: ActionsForm; waiting: boolean }) {
  const key = detail.node.key;
  const mutation = actMutation(act, key);
  const run = () => void write.run(mutation === undefined ? [] : [mutation]);
  switch (act) {
    case "done":
      return (
        <Button primary disabled={write.disabled || waiting} onClick={run}>
          Mark done
        </Button>
      );
    case "done-with-evidence":
      return <DoneButton disabled={write.disabled} onOpen={() => { form.openForm("evidence", { artifact: "", note: "" }); }} />;
    case "start":
      return (
        <Button disabled={write.disabled} onClick={run}>
          Start
        </Button>
      );
    case "reach":
      return <Reach write={write} node={key} today={view.derived.today} disabled={waiting} />;
    case "reach-now":
      return (
        <Button primary disabled={write.disabled} onClick={run}>
          Confirm reached now
        </Button>
      );
    case "break-down":
      return (
        <Button onClick={() => { form.openPanel("break-down"); }} data-testid="break-down">
          Break down
        </Button>
      );
    case "mark-atomic":
      return (
        <Button disabled={write.disabled} onClick={run}>
          Mark atomic
        </Button>
      );
    case "unsnooze":
      return (
        <Button disabled={write.disabled} onClick={run}>
          Unsnooze
        </Button>
      );
  }
}

/**
 * An earlier version of the form landed; this is what was typed after it, which was not sent.
 * The action already happened, so nothing here would do it again: the text can be read and
 * copied, and Close ends it.
 */
function Kept({ opened, view, detail, onClose }: { opened: Drafted; view: Ready; detail: NodeDetail; onClose: () => void }) {
  const text = (value: ReactNode) => <span className="muted" data-testid="kept-text">{value}</span>;
  const typed = (): ReactNode => {
    switch (opened.case) {
      case "snooze":
        return text(snoozeWords(view, opened.value));
      case "evidence":
        return (
          <>
            {opened.value.artifact === "" ? null : text(opened.value.artifact)}
            {opened.value.note === "" ? null : text(opened.value.note)}
          </>
        );
      default:
        return text(`\u201c${opened.value}\u201d`);
    }
  };
  return (
    <div className="callout stack" data-testid="kept">
      <span>
        <strong>{keptWords(opened.case, detail.node.kind)}</strong> What you typed after sending was not sent:
      </span>
      {typed()}
      <span>
        <Button onClick={onClose}>Close</Button>
      </span>
    </div>
  );
}

interface OpenProps {
  view: Ready;
  detail: NodeDetail;
  write: NodeWrite;
  form: ActionsForm;
  sending: Sending;
  /** Sends the form's write and tells the form which write it sent, at which version of its draft. */
  submit: (opened: Drafted, mutations: Mutation[]) => void;
}

type ReasonKind = "skip" | "keep" | "include" | "bypass";

/** What sending a reason form's `reason` does to `node`: D1's skip, B5's keep and force include, D4's guard bypass with the move it was needed for. */
function reasonMutations(kind: ReasonKind, node: GraphNode, guards: Guard[], reason: string): Mutation[] {
  switch (kind) {
    case "skip":
      return [transition(node.key, "skip", reason)];
    case "keep":
      return [{ op: "apply_override", node: node.key, override: { keep: { reason } } }];
    case "include":
      return [{ op: "apply_override", node: node.key, override: { force_include: { reason } } }];
    case "bypass":
      return [{ op: "apply_override", node: node.key, override: { guard_bypass: { guards, reason } } }, transition(node.key, node.kind === "milestone" ? "reach" : "complete")];
  }
}

/** The field's label and the confirming button's words of a reason form. */
function reasonWords(kind: ReasonKind, node: GraphNode, without: string): { label: string; confirm: string; testId: string } {
  switch (kind) {
    case "skip":
      return { label: "Why skip it", confirm: "Skip", testId: "skip-form" };
    case "keep":
      return { label: "Why keep it", confirm: "Keep", testId: "keep-form" };
    case "include":
      return { label: "Why include it", confirm: "Include anyway", testId: "include-form" };
    case "bypass":
      return { label: "Why bypass the guard", confirm: `${node.kind === "milestone" ? "Mark reached" : "Mark done"} without ${without}`, testId: "bypass" };
  }
}

/** The forms that take a reason. */
function ReasonCase({ view, detail, form, write, sending, submit, opened }: OpenProps & { opened: Extract<Drafted, { case: ReasonKind }> }) {
  const { node } = detail;
  const guards = guardsOf(view, detail);
  return (
    <ReasonForm
      {...reasonWords(opened.case, node, guards.words)}
      value={opened.value}
      sending={sending}
      disabled={write.disabled}
      onChange={(next) => { form.change(opened.case, next); }}
      onConfirm={(reason) => { submit(opened, reasonMutations(opened.case, node, guards.guards, reason)); }}
      onClose={form.close}
    />
  );
}

/** The form or panel that is open, in place of the row; the snooze dialog shows `footer` (the write's rejection) inside itself. */
function OpenedForm({ opened, footer, ...props }: OpenProps & { opened: Opened; footer: ReactNode }) {
  const { view, detail, write, form, sending, submit } = props;
  const { node } = detail;
  const shared = { sending, disabled: write.disabled, onClose: form.close };
  switch (opened.case) {
    case "skip":
    case "keep":
    case "include":
    case "bypass":
      return <ReasonCase {...props} opened={opened} />;
    case "anyway":
      // The reason is typed inside the answer form, which sends it with the answer.
      return null;
    case "snooze":
      return <SnoozePanel {...shared} view={view} detail={detail} value={opened.value} footer={footer} onChange={(next) => { form.change("snooze", next); }} onSnooze={(mutation) => { submit(opened, [mutation]); }} />;
    case "evidence":
      return <EvidenceForm {...shared} needs={missingEvidence(view, node)} value={evidenceDraft(opened.value)} onChange={(next) => { form.change("evidence", next); }} onSend={(evidence) => { submit(opened, doneMutations(node.key, evidence)); }} />;
    case "edit-date":
      return <EditDate {...shared} value={opened.value} onChange={(next) => { form.change("edit-date", next); }} onSave={(date) => { submit(opened, [{ op: "set_recorded_date", node: node.key, end: "finish", date }]); }} />;
    case "assign":
      return (
        <span className="row">
          <AssignOwner view={view} write={write} node={node.key} />
          <Button onClick={form.close}>Close</Button>
        </span>
      );
    case "weight":
      return (
        <>
          <WeightEditor view={view} detail={detail} />
          <span>
            <Button onClick={form.close}>Close</Button>
          </span>
        </>
      );
    case "break-down":
      return <BreakDown ready={view} node={node} onCancel={form.close} />;
  }
}

/**
 * What the graph's context menu chose for this node: it selects the node and leaves the choice in
 * the navigation's state, which the node's own Actions carry out once, with the handlers its `⋯`
 * and buttons use. The choice is taken off the history entry as it is run, so a reload does not repeat it.
 */
function useContextChoice(node: string, run: (id: ContextId) => void): void {
  const navigate = useNavigate();
  const location = useLocation();
  const frame = useFrameActions();
  const chosen = (location.state as { choice?: { node: string; id: ContextId } } | null)?.choice;
  const done = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (chosen === undefined || chosen.node !== node || done.current === location.key) {
      return;
    }
    done.current = location.key;
    // The form or the move is shown in the inspector, which the viewer may have on the assistant's tab or hidden.
    frame?.show("inspector");
    void navigate(location, { replace: true, state: { ...(location.state as object), choice: undefined } });
    run(chosen.id);
  });
}

/**
 * The `⋯` menu's groups and what each entry does, and the choice the graph's context menu left
 * for this node (done with the same handlers).
 */
function useMenu(menus: Omit<Menus, "insertHere">): { groups: MenuItem[][]; runners: Record<MenuId, () => void> } {
  const { view, detail, write, form } = menus;
  const navigate = useNavigate();
  const { search } = useLocation();
  const insertHere = useInsertEntry();
  // Only a frame with a stepper can insert: elsewhere the entry is not offered.
  const groups = menuOf(view, detail)
    .map((group) => group.filter((item) => item.id !== "insert-segment" || insertHere !== undefined))
    .filter((group) => group.length > 0);
  const runners = runnersOf({ ...menus, insertHere }, (to, state) => void navigate(to, state === undefined ? {} : { state }), search);
  useContextChoice(detail.node.key, (id) => {
    if (id in runners) {
      runners[id as MenuId]();
      return;
    }
    if (id === "done-with-evidence") {
      // Opening the open case again keeps what was typed; another case takes the form's place.
      form.openForm("evidence", { artifact: "", note: "" });
    } else {
      // What the row itself shows (its buttons and the answer form) is in place of any form the menu opened.
      form.close();
      const mutation = actMutation(id as Act, detail.node.key);
      // As the row's buttons are, not offered again while a write is in flight.
      if (mutation !== undefined && !write.disabled) {
        void write.run([mutation]);
      }
    }
  });
  return { groups, runners };
}

export function Actions({ view, detail, onRename }: { view: Ready; detail: NodeDetail; onRename: () => void }) {
  // A fresh machine per node: its form, its draft and its watch are the node's.
  return <NodeActions key={`${view.journey.header.id}:${detail.node.key}`} view={view} detail={detail} onRename={onRename} />;
}

function NodeActions({ view, detail, onRename }: { view: Ready; detail: NodeDetail; onRename: () => void }) {
  const write = useNodeWrite(view, `actions:${detail.node.key}`, detail.node.key);
  const { node, record } = detail;
  const [copied, setCopied] = useState(false);
  const form = useActionsForm(write.journey, node.key, write.seen, () => { setCopied(false); });
  const offers = offered(view, detail);
  const deciding = answerable(node.kind, record.state);
  const { open } = form;
  const kept = form.view.state === "kept" ? form.view.opened : undefined;
  // Open in place of the row: every form and panel but the answer form's own reason.
  const inPlace = open !== undefined && open.case !== "anyway" && kept === undefined;
  const showsAnswer = deciding && !inPlace && kept === undefined;
  const showsRow = !deciding && !inPlace && kept === undefined;
  const out = form.view.state === "sending" && write.pending;
  const sending: Sending = !out ? false : write.status === undefined || write.status.state === "sending" ? "saving" : "waiting";
  /** A form's submit: the write goes to the edit manager, and the form is told which write it sent, at which version of its draft. */
  const submit = (from: Drafted, mutations: Mutation[]) => {
    const id = newPatchId();
    void write.run(mutations, from.seen, id);
    if (write.holds(id)) {
      form.sent(id, from.version);
    }
  };
  const anyway: Anyway | undefined =
    open?.case === "anyway" && kept === undefined
      ? {
          reason: open.value,
          change: (next) => { form.change("anyway", next); },
          cancel: form.close,
          sent: (id) => { form.sent(id, open.version); },
          resent: form.resent,
        }
      : undefined;
  const { groups, runners } = useMenu({ view, detail, write, form, onRename, onCopied: () => { setCopied(true); } });
  const menu = <Overflow groups={groups} runners={runners} />;
  const acts = [...(offers.primary === undefined ? [] : [offers.primary]), ...offers.secondary];
  const moving = movesFrom(node.kind, record.state).includes("start") ? "Starting" : "Finishing";
  const rejected = <Rejected view={view} write={write} onResent={form.resent} />;
  const inDialog = open?.case === "snooze";
  return (
    <div className="stack" data-testid="actions">
      {showsAnswer ? <AnswerEditor view={view} detail={detail} menu={menu} anyway={anyway} /> : null}
      {showsAnswer && offers.secondary.includes("unsnooze") ? (
        <span>
          <ActButton act="unsnooze" view={view} detail={detail} write={write} form={form} waiting={false} />
        </span>
      ) : null}
      {showsRow ? (
        <>
          {offers.primary === undefined ? null : <LiftsSnooze view={view} node={node.key} moving={moving} />}
          <span className="row actions-row">
            {acts.map((act) => (
              <ActButton key={act} act={act} view={view} detail={detail} write={write} form={form} waiting={offers.waiting} />
            ))}
            <span className="spacer" />
            {menu}
          </span>
        </>
      ) : null}
      {inPlace ? <OpenedForm view={view} detail={detail} write={write} form={form} opened={open} sending={sending} footer={inDialog ? rejected : null} submit={submit} /> : null}
      {kept === undefined ? null : <Kept opened={kept} view={view} detail={detail} onClose={form.close} />}
      {copied ? <span className="muted small">Link copied.</span> : null}
      {kept === undefined && !inDialog ? rejected : null}
    </div>
  );
}
