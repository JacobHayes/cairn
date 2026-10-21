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
import { doneMutations, evidenceDraft, missingEvidence } from "../acting/acts.ts";
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
import { ActButton, actMutation, type Opens } from "./ActButton.tsx";
import { EvidenceForm } from "./DoneEvidence.tsx";
import { SnoozePanel, snoozeWords, WeightEditor } from "./editors.tsx";
import { foldKey, setFold } from "./folds.ts";
import { answerable, failingGuards, guardBypass, movesFrom, storedSnooze, titleOf, transition, type Guard, type GraphNode, type Mutation, type NodeDetail, type Ready } from "./model.ts";
import { menuOf, offered, type Act, type ContextId, type MenuId, type MenuItem } from "./offers.ts";
import { OwnerSelect } from "./parts.tsx";
import { Rejected } from "./Rejected.tsx";
import { snoozeStart } from "./snooze.ts";
import { Saving, type Sending } from "../ui/Receipt.tsx";
import { guardWords, keptWords } from "./words.ts";
import { useNodeWrite, type NodeWrite } from "./write.ts";
import { plural } from "../ui/words.ts";

/**
 * D4: beside a finishing action on undecided work, that finishing it is accepted but may not
 * apply, naming the decisions it waits on; nothing when the node is not undecided.
 */
export function MayNotApply({ view, node }: { view: Ready; node: string }) {
  const unanswered = view.derived.nodes[node]?.relevance.unanswered ?? [];
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
  if (held === 0 || !storedSnooze(view, node)) {
    return null;
  }
  return (
    <span className="muted" data-testid="lifts-snooze">
      {moving} this lifts the snooze on {titleOf(view, node)} and its {plural(held, "open item")}.
    </span>
  );
}

/** The guards a bypass would pass, with what the node is missing in words. */
function guardsOf(detail: NodeDetail): { guards: Guard[]; words: string } {
  const guards = failingGuards(detail.derived);
  return { guards, words: guards.map(guardWords).join(" and ") };
}

/** F2: a done milestone's actual date, edited. */
function EditDate({ value, sending, disabled, onChange, onSave, onClose }: { value: string; sending: Sending; disabled: boolean; onChange: (date: string) => void; onSave: (date: string) => void; onClose: () => void }) {
  return (
    <span className="stack" data-testid="edit-date-form">
      <span className="row row-end">
        <label className="stack">
          <span className="muted">Done on</span>
          <Field type="date" aria-label="Done on" value={value} onChange={(event) => { onChange(event.target.value); }} />
        </label>
        <Button primary disabled={disabled || value === ""} onClick={() => { onSave(value); }}>
          Save
        </Button>
        <Dismiss sending={sending} onClose={onClose} />
      </span>
      <Saving sending={sending} />
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
    <Menu label="More actions" title="More actions" testId="more-actions" align="end" trigger={<span aria-hidden="true">⋯</span>}>
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

/**
 * An earlier version of the form landed; this is what was typed after it, which was not sent.
 * The action already happened, so nothing here would do it again: the text can be read and
 * copied, and Close ends it.
 */
function Kept({ opened, view, onClose }: { opened: Drafted; view: Ready; onClose: () => void }) {
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
        <strong>{keptWords(opened.case)}</strong> What you typed after sending was not sent:
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
      return [guardBypass(node.key, guards, reason), transition(node.key, "complete")];
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
      return { label: "Why bypass the guard", confirm: `Mark done without ${without}`, testId: "bypass" };
  }
}

/** The forms that take a reason. */
function ReasonCase({ detail, form, write, sending, submit, opened }: OpenProps & { opened: Extract<Drafted, { case: ReasonKind }> }) {
  const { node } = detail;
  const guards = guardsOf(detail);
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
        <span className="row" data-testid="assign">
          <OwnerSelect view={view} write={write} node={node.key} placeholder="Assign owner" />
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
 * What the graph's context menu, or a row's Decide, chose for this node: it opens the node and
 * leaves the choice in the navigation's state, which the node's own Actions carry out once they
 * are drawn (however long the screen takes to load), with the handlers its `⋯` and buttons use.
 * The choice is taken off the history entry as it is run, so a reload does not repeat it.
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
 * The `⋯` menu's groups and what each entry does, and the choice the graph's context menu or a
 * row's Decide left for this node (done with the same handlers), and whether Decide asked the
 * answer form for the focus (`focusAnswer`, which the form calls once it has taken it).
 */
function useMenu(menus: Omit<Menus, "insertHere">): { groups: MenuItem[][]; runners: Record<MenuId, () => void>; focusAnswer: (() => void) | undefined } {
  const { view, detail, write, form } = menus;
  const navigate = useNavigate();
  const { search } = useLocation();
  const insertHere = useInsertEntry();
  // C11: Decide shows the answer form in place of any other form and asks it for the focus, which it takes once drawn.
  const [focusAsked, setFocusAsked] = useState(false);
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
    if (id === "decide") {
      form.close();
      setFocusAsked(true);
    } else if (id === "done-with-evidence") {
      // Opening the open case again keeps what was typed; another case takes the form's place.
      form.openForm("evidence", { artifact: "", note: "" });
    } else {
      // What the row itself shows (its buttons and the answer form) is in place of any form the menu opened.
      form.close();
      const mutation = actMutation(view, id as Act, detail.node.key);
      // As the row's buttons are, not offered again while a write is in flight.
      if (mutation !== undefined && !write.disabled) {
        void write.run([mutation]);
      }
    }
  });
  return { groups, runners, focusAnswer: focusAsked ? () => { setFocusAsked(false); } : undefined };
}

/** What the form's buttons open here: the done form and the break-down panel (the answer form is drawn in place of Decide). */
function opensIn(form: ActionsForm): (what: Opens) => void {
  return (what) => {
    if (what === "evidence") {
      form.openForm("evidence", { artifact: "", note: "" });
    } else if (what === "break-down") {
      form.openPanel("break-down");
    }
  };
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
  const offers = offered(view, node.key);
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
  const { groups, runners, focusAnswer } = useMenu({ view, detail, write, form, onRename, onCopied: () => { setCopied(true); } });
  const menu = <Overflow groups={groups} runners={runners} />;
  // A decision awaiting its answer has the answer form in the place of Decide.
  const acts = [...(offers.primary === undefined || offers.primary === "decide" ? [] : [offers.primary]), ...offers.secondary];
  const opens = opensIn(form);
  const moving = movesFrom(node.kind, record.state).includes("start") ? "Starting" : "Finishing";
  const rejected = <Rejected view={view} write={write} onResent={form.resent} />;
  const inDialog = open?.case === "snooze";
  // With nothing to do but the overflow menu, it sits beside the sentence above it instead of on a row of its own.
  const menuOnly = showsRow && acts.length === 0 && groups.length > 0 && !showsAnswer && !copied;
  return (
    <div className={menuOnly ? "stack actions-menu-only" : "stack"} data-testid="actions">
      {showsAnswer ? <AnswerEditor view={view} detail={detail} menu={menu} anyway={anyway} focus={focusAnswer} /> : null}
      {showsAnswer && offers.secondary.includes("unsnooze") ? (
        <span>
          <ActButton act="unsnooze" view={view} node={node.key} write={write} waiting={false} open={opens} />
        </span>
      ) : null}
      {showsRow && (acts.length > 0 || groups.length > 0) ? (
        <>
          {offers.primary === undefined ? null : <LiftsSnooze view={view} node={node.key} moving={moving} />}
          <span className="row actions-row">
            {acts.map((act) => (
              <ActButton key={act} act={act} view={view} node={node.key} write={write} waiting={offers.waiting} open={opens} dated />
            ))}
            <span className="spacer" />
            {menu}
          </span>
        </>
      ) : null}
      {inPlace ? <OpenedForm view={view} detail={detail} write={write} form={form} opened={open} sending={sending} footer={inDialog ? rejected : null} submit={submit} /> : null}
      {kept === undefined ? null : <Kept opened={kept} view={view} onClose={form.close} />}
      {copied ? <span className="muted small">Link copied.</span> : null}
      {kept === undefined && !inDialog ? rejected : null}
    </div>
  );
}
