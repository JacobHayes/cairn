# Cairn: PRD

## Summary

Cairn structures a process as one graph of **nodes** to be decided or done: decisions (typed questions), deliverables, actions, milestones, and groups. Nodes nest (containment), depend on each other (`requires`), and are filtered by answers (conditions), which also decide who is involved and when things are due. A **route** is a versioned, stateless template; a **journey** is a live graph with state, copied from a route version or started empty, and savable as a route. Every change is a validated, atomic **patch**. Cairn ranks what can move next. People use a web UI; agents use an HTTP API and a remote MCP server with the same capabilities, and an in-app assistant uses the same tools. Conversation with an assistant is the expected main authoring path; the canvas and file import are the others, all producing the same patches.

## Goals

1. A process encoded once as a route, entirely as data: a new process or deployment needs no code change.
2. A journey always answers: where am I, what's next, what needs deciding, what's blocked, what did I forget.
3. A ranked frontier whose ranking explains itself.
4. Ad-hoc journeys with no route, structured as they go, savable as routes.
5. Involvement explicit: decisions fill roles, nodes inherit participation, any node overrides.
6. Multiplayer, every change attributed and logged.
7. AI-native: agents can do everything the UI does, remotely, with no install.

## Non-goals (for now)

- Access control: every authenticated user reads and writes everything in a deployment.
- Project management: no capacity, sprints, leveling, or auto-scheduling. Feasibility is checked, never scheduled.
- Hosting artifacts (Cairn stores links) or doing the work, including sending messages.
- Automatic sync between route files and the database.
- Tolerating invalid structure: a patch that breaks an invariant is rejected.
- Learned priority: weights are authored and ranking is a fixed formula.

## Core concepts

The glossary: one meaning per term, used in code and docs.

| Term | Meaning |
|---|---|
| **Deployment** | One installation and one trust boundary: users, identities, routes, journeys. |
| **Route** | A named template graph: immutable numbered **versions** and at most one **draft**; no state. It starts a journey or is inserted into a graph. |
| **Insertion** | Copying a route version into a graph with fresh keys. Avoid: template, snippet, module, sub-route. |
| **Journey** | A live graph with state: answers, node states, participations, pins, snoozes, notes, links, owners, history. Status `active`, `completed`, or `archived`. |
| **Journey owner** | An identity responsible for a journey. Not a participation; never saved into a route. |
| **Lineage** | A pointer to the route version a journey or an inserted root was copied from. Nothing follows the route afterwards. |
| **Node** | The one recursive unit, of kind `decision`, `deliverable`, `action`, `milestone`, or `group`. |
| **Group** | A pure container, never actionable; its state is derived unless marked done or skipped. A top-level group is a **stage**, with no fields of its own. |
| **Answer** | A journey's value for a decision, with an optional **rationale**. An empty value is an answer; no submission is unanswered. |
| **Edge** | A hard dependency, `requires`. |
| **Condition** | `relevant_when`, a predicate over answers deciding relevance. |
| **Role** | A single- or multi-valued slot filled per journey with identities, possibly by one **filling decision**. |
| **Participation** | A node's `{kind -> source}`, the source a role or explicit identities. `owner` is the one built-in kind and a decision's decider; other kinds are informational. |
| **Identity** | A person or team, with a name, emails, or both. Not a login: most identities a journey names never sign in. |
| **User** | An authenticated account, holding one or more **logins**; agents act for a user. |
| **Pin** | An explicit date on a node, optionally with a time of day. |
| **Date rule** | `due_by` or `not_before`, relative to a milestone, a date answer, or the journey's `created_at`. |
| **Due / earliest start / latest start / slack** | Derived date bounds; slack is latest start minus today. |
| **Shortfall** | A derived warning that the plan ahead cannot be met. |
| **Gravity / unlocks / rank** | Derived priority: the weight riding on a node, the weight finishing it frees now, and their blend with urgency. |
| **Frontier** | The actionable nodes with no actionable descendants. The **acting frontier** leaves out snoozed and scheduled ones, and drives what's next. |
| **Actionable** | Relevant, not blocked, not terminal. |
| **Terminal** | `done`, `skipped`, or `decided`. |
| **Display state** | The one derived status every surface shows. |
| **Guard** | A completion precondition on evidence, bypassable. |
| **Stale** | Finished work to revisit. |
| **Placeholder** | Work each journey must break down or mark `atomic`. |
| **Structural change** | A change to what the graph is (nodes, edges, roles, kinds, conditions, date rules, resources). Everything else a journey holds is state. |
| **Patch** | Ordered mutations applied atomically against a base revision; the only write. |
| **Revision** | A monotonic number per domain (a journey, a route with its draft, the deployment) and per proposal. |
| **Proposal** | A patch drafted for review, not applied. |
| **Consequences** | What a patch newly caused in derived state. |
| **Notice** | An advisory finding on a route graph; never a rejection. |
| **Event** | An append-only record of one mutation. |

### The three relationships

Containment (a tree), dependency (`requires`, acyclic, never between a node and its ancestor or descendant), and relevance (conditions) are independent, except where containment adds dependencies.

### Containment

- **A parent requires each relevant or undecided child.** A parent's own work comes after its children; until then its actionable children are the frontier. A group is done when its children and its own dependencies are satisfied; an empty group still waits for its gates.
- **Children inherit their parent's dependencies**: a `requires` into a node gates every descendant.
- These implicit edges, and the gates conditions add, count everywhere an explicit edge does (blocking, traces, gravity, dates, cycle checks) and are named by their source in every explanation.

### Keys and references

- Nodes, roles, and participation kinds have an **id** (a slug unique among siblings) and a stable opaque **key** (`n_7q4m2k`), minted once and kept through renames, moves, copies into journeys, and exports. A node's **path** is its ids joined from the root (`testing/report`).
- References are stored by key, so renames and moves break nothing; paths and ids are for files and display.
- Inserting a route is the one copy that mints keys, each derived from the insertion key and the route key, so a route can be inserted twice.

### Gating: how decisions sit in the graph

- An ungated decision is actionable at once; one that `requires` work waits for it, so roles and dates can be settled late.
- A node that `requires` a decision, or reads it in a condition, waits for its answer.
- **Relevance** is `relevant`, `not_relevant`, or `undecided`, combining a node's condition with its ancestors' (`not_relevant` beats `undecided` beats `relevant`). Conditions are three-valued: only an open, relevant decision makes a node undecided, so nothing waits forever on a decision nobody will make. A skipped or ruled-out decision is unanswered, and every value test on it is false.
- **Blocked** means a dependency is unsatisfied. Blocking takes a node off what's next but never stops a move: any node can be started, completed, answered, or annotated early, and work done ahead reads like any other.
- **Force include** (with a reason) makes a node relevant despite conditions. There is no force skip: skip the node.

### Priority

Work is ranked by how soon it's due, how much depends on it, and how much it frees for other people. Every node can say why it ranks where it does.

## Functional requirements

### A. Graph definition (routes and journeys share this)

- **A1** A graph is entirely data in a schema-validated document.
- **A1a** A node's fields are fixed per kind; journey-only state (such as `atomic`) never appears on a route.
- **A2** Containment is a tree of any depth. Decisions and milestones are leaves.
- **A3** `requires` joins any two nodes regardless of kind or depth, except a node and its ancestor or descendant.
- **A4** A decision has a prompt, an answer type (boolean, single choice, multi choice, text, date, identity, identity list), and optional choices and help. It may fill a role or feed a milestone's date.
- **A5** Conditions read decisions with a fixed operator set, three-valued, and apply to descendants.
- **A6** A role is single or multi; a filling decision's answer type matches its cardinality, and a role has at most one. A graph may name a `default_owner` role.
- **A7** A graph declares its own informational participation kinds, single or multi.
- **A8** Date rules give a node a latest finish (`due_by`) or an earliest start (`not_before`), offset in days from their sources. The own `not_before` of a deliverable, action, or decision holds it scheduled while ahead; acting earlier is a plain move, never guarded and never a shortfall. A container's rule holds only its own work.
- **A9** Dates, estimates, and offsets are calendar days in the deployment's time zone. A pin, a date answer, or a snooze may carry a time of day; a time acts only on the node that holds it, and everything derived from it stays in days.
- **A10** Nodes carry resources: tips, typed links, and message drafts whose placeholders render from the journey. Anything unfilled shows a visible marker; rendering never fails.
- **A11** A route has immutable numbered versions and at most one draft, which extends the latest version and is what patches target. Publishing makes the next version. Opening a second draft is refused, offering to discard the first. Route edits never reach a journey.
- **A12** Routes are authored in conversation, on the canvas, or by file import, all as patches against the draft.
- **A13** Routes import and export one canonical file, round-trip safe.
- **A14** Graphs are stored as structured, queryable records with constraints; the file only serializes them.
- **A15** A patch that would break an invariant is rejected with every violation by path. Notices are the only advisory findings.
- **A16** Completion is gated only by evidence a node asks for (an artifact, a note, a ticked checklist, a breakdown) and by its own children.
- **A17** A patch targets one domain (a journey, a route with its draft, or the deployment), or a proposal in it, names its base revision, and commits all its mutations and events or none. Transitions are checked at each mutation's position; invariants and guards once, on the result, so a bulk change is accepted in any order. Creating a journey or route is its first patch. Creating an identity may ride in a patch to any domain, so one write can create a person and reference them; otherwise only hard deletion spans domains.
- **A18** Removing a node removes its subtree and incident edges; any other reference to it must be rewritten or removed in the same patch. A removal names everything it removes and is refused if that set has grown, so a stale removal never widens.
- **A19** A draft can be discarded, a route retired (closed to new journeys and insertions), and a journey archived. Hard deletion is a deployment patch naming each journey and route at the revision read, all or none; a route can be deleted only while no journey's lineage names it. Deleted ids are never reused.
- **A20** The one notice: in a graph with a `final` milestone, a node with no chain to or from it (groups and role-filling decisions aside). Journeys are not checked.

### B. Starting and shaping a journey

- **B1** A journey starts from a route version or empty, with a name, optional description, and owners (none by default). Nodes start in their kind's initial state.
- **B2** A decision can be answered whenever it is in scope or undecided, ahead of its dependencies or date. Answers are revisable and logged; a rationale belongs to its answer, and reopening clears both. State on nodes an answer rules out is kept.
- **B3** Filling a role picks identities or creates them by name.
- **B4** A journey edits its own structure freely. Its nodes are its own once copied: nothing is merged back or forward.
- **B5** Participations, pins, and weights are ordinary journey state. Force include and keep are overrides with a reason. All survive role and answer changes.
- **B6** Snooze an actionable node or a container until a date or until another node completes. A container's snooze holds its subtree. Holding is derived on read, so a node snooze holds again if its target reopens. Any transition on the snoozed node clears its snooze. Snoozed nodes still block and count for gravity, but leave what's next.
- **B7** A journey's lineage is shown and changes nothing: a newer route version leaves the journey as it is.
- **B8** Save as route: a journey's structure (no state, answers, notes, or owners) becomes a new route draft or a new draft of an existing route, keys kept. Review maps each named identity to a role or drops it, and can exclude nodes.
- **B10** Breakdown: deliverables and actions gain children by hand or by proposal, including one child per role member. A placeholder needs breakdown until it has children or is marked `atomic`.
- **B11** Completing a journey is the user's call; Cairn suggests it when a journey with nodes has nothing in scope left, or its `final` milestone is done. A completed journey stays editable and can reopen. An archived one accepts only a status move.
- **B13** Insert a route version into a journey or route draft, under a container or at the top. Its roles and kinds map onto the graph's or are added; nodes can be left out. Each inserted root keeps a lineage. In a journey the copies start in initial states.
- **B15** Save a selection as a route: the selected subtrees become a new route draft with the review of save as route. Nothing outside is carried, and every reference cut at the boundary is listed.

### C. Graph, navigation, and views

What a person can see and do; how it looks is DESIGN's.

- **C1** Canvas: a journey's graph with pan and zoom. A card shows kind, title, display state, its most actionable date, and its owner where it matters. Date relationships are drawn only for the selected node. Not-relevant nodes are hidden by default, and the Filter always counts what it hides, so hiding never hides a problem.
- **C2** Every view of a journey can be narrowed by depth (stages, decisions, deliverables, actions), filter, and search, and the setting carries across views. Hidden nodes fold into their nearest visible container, and hiding never makes blocked work look free. The selected node is always drawn.
- **C3** At full depth, with nothing filtered, the whole graph is one canvas.
- **C4** Drilling in and out expands and collapses containers in place.
- **C5** "I am here": the acting frontier, active work, and the viewer's own items stand out; the current stages open first.
- **C6** Priority is visible but quiet: dates and rank are shown in words, never by weight, opacity, or motion.
- **C7** Trace: selecting a node marks everything upstream and downstream of it, across levels, in words.
- **C8** Node detail explains a node: its state and what it means, its answer and each choice's effect, its dates and their chains, its signals and contributors, why it is relevant or blocked, and its history in sentences that name people, never ids.
- **C9** List view: the drawn nodes as a table, filterable, searchable, and sortable by any signal. Bulk actions are one patch; any failing node fails it, named.
- **C10** Frontier: a highlight on every view, not a page. Each row gives one reason in words and its primary action. It matches the agent's next-up list.
- **C11** Triage (the walkthrough): the acting frontier one card at a time in rank order, each card the node's detail with the actions its kind allows. Passing reorders only this pass and writes nothing. After an action, what it unlocked comes next. At decision depth it is the decision walkthrough, which a new journey opens on.
- **C12** Decision view: decisions, their groups, milestones, and gating edges; a projection, not a structure.
- **C13** Timeline: dated nodes on a time axis, overdue and shortfall marked, the `final` milestone as end anchor. A route's timeline is relative, starting at day 0.
- **C14** Proposal review: a proposal is a layer over its destination's own views, changes marked in place, with an editable list of its items. Validation runs on the final result at apply. While the layer is on, the page takes no live write. Apply is enabled once nothing holds it and is sent at the proposal revision shown.
- **C15** Layout is automatic and deterministic; small edits move few nodes.
- **C16** Every journey, node, edge, and proposal has one address, with the view in the query. Home lists the viewer's work that can move now and the proposals waiting on them. A journey's front page is its summary.
- **C17** A route page shows the draft if open, else the latest version, with any version selectable; the draft is editable there and versions are read-only.
- **C18** Journey summary: what is left and what is in the way of it, in lists that each open in full.
- **C19** Insert a route from a journey, container, or draft, reviewed as a proposal. Save a selection as a route from a canvas selection.
- **C20** A guided tour, offered once on Home and again on request, on a practice journey made for the person and deleted at the end.

### D. Node state and lifecycle

- **D1** Each non-group kind has a fixed state machine:

  | Kind | States | Transitions |
  |---|---|---|
  | deliverable, action | `todo`, `active`, `done`, `skipped` | start, stop, complete, skip, reopen |
  | decision | `open`, `decided`, `skipped` | answer, revise, skip, reopen (clears the answer and what it filled) |
  | milestone | `pending`, `done`, `skipped` | complete (records the actual date), skip, reopen |
  | group | derived | skip, mark done, reopen |

  Skipping and marking done need a reason. Reopening is always allowed. Completing a parent leaves its children open.
- **D1a** Skipping a container, or marking a group done, effectively skips its unfinished descendants without touching their stored states; reopening restores them. `keep` exempts a subtree, and its container then frees dependents only once the kept work is done.
- **D2** Actionable: relevant, not blocked, not terminal, never a group. Ownership does not matter.
- **D3** Every derived value (relevance, blocking, dates, priority, flags, progress, display state, the moves a node accepts) is computed from stored state on read and never stored.
- **D4** Guards check a completion's evidence against the graph the patch produces; they are never standing invariants, so later changes make work `stale` instead. Order is never a guard: nothing waits on a dependency or a date to start or finish. A not-relevant node cannot complete; an undecided one completes with a warning. `complete_anyway` bypasses failing guards and records them. Finished work is `stale` when a dependency it relied on reopens, or when its guards fail in a way its bypass did not record.
- **D5** `stalled`: nothing can move now, but in-scope work remains. The diagnostic names what it waits on and offers what is allowed.
- **D6** Any cache of derived values is invisible.
- **D7** Every accepted patch and proposal preview reports its consequences, with explanations, never stored. Warnings: new stale work, new or larger shortfalls, newly overdue, finished work newly undecided, newly stalled. Information: what came onto the acting frontier, left scope, or entered it. People see the warnings; agents get both.
- **D8** Display state, first match wins: `not_relevant`, `skipped`, `done`, `snoozed`, `active`, `conditional`, `blocked`, `scheduled`, `ready`. A container is never blocked by its own children. A node that rests on an undecided decision is conditional, not ruled out. Stored state is secondary, and what transitions act on.

### E. Roles and participation

- **E1** `owner` is the one built-in kind, one identity. In-scope unfinished work with no owner is `unassigned`, and acting surfaces offer assignment inline.
- **E2** A node's participation is its own, else its role's, else its nearest ancestor's, else `default_owner` for the owner. Multi-valued kinds replace, never union. An explicit empty value means none.
- **E3** One authority per role: a role with a filling decision is filled only through it, and empties when the decision is reopened, skipped, or leaves scope. A fed milestone's pin is likewise set only through its decision.
- **E4** "Mine": the nodes where the viewer's identity takes part, and the active journeys they own or work in.
- **E5** Changing a role's identities re-derives inherited participations and keeps explicit ones.
- **E6** People and teams are identities. Two can be merged, and the old one keeps working as an alias. One can be deleted only when nothing refers to it. A merge or delete that would break a journey is refused, naming what it would break.

### F. Dates and milestones

- **F1** A milestone's **effective date** is its actual date, else its pin, else its due. An `auto_done` milestone is done, with no event, once its date comes and its dependencies are satisfied; other milestones are marked done. A milestone is scheduled only by a date it is held to (a pin, a fed answer, or its own date when it marks itself done), never by a deadline derived for it.
- **F2** One constraint network: every timing relation (date rules, dependencies, estimates, containment) is "B is at least k days after A". Not-relevant nodes drop out; undecided ones stay, labelled conditional.
- **F3** Every bound is the tightest the network allows and carries its chain ("decision meeting Oct 10, final review due by it, report takes 3 days"). A bound nothing reaches is empty. A skipped milestone's date bounds nothing.
- **F4** A stage is an ordinary group: it waits on a milestone by requiring it, and its deadline is its `due_by`.
- **F5** Plan consistency is an invariant: no constraints that need a date before itself. A patch creating such a chain is rejected listing the chains, each with the fixes it allows (shift, repin, unpin, change an offset or estimate), offered inline. Nothing shifts automatically.
- **F6** Reality is never rejected, and recorded dates never warn. Actuals and today produce flags instead: `overdue`, and `shortfall` when work ahead cannot fit. Finishing early, late, or out of order is never a shortfall.
- **F7** Every surface tells pinned, actual, and derived dates apart and names the pin or rule that moves a bound.

### G. Notes, links, and artifacts

- **G1** Any node, in any state, and the journey take markdown notes and typed links: attributed, editable, and logged.
- **G2** An artifact link satisfies only its own node's artifact guard.
- **G3** Message drafts render with journey context and can be copied.
- **G4** A required note is a note on the node itself.
- **G5** A checklist is ordered tickable items on a deliverable or action. Ticks are journey state and survive reopening. A required checklist fails once per unticked item.

### H. Multiplayer and identity

- **H1** Authentication is pluggable: OAuth or OIDC providers in production, a token or named user locally.
- **H2** Every mutation records its actor, a user or an agent for one; applying a proposal also records who confirmed it.
- **H3** A user is the identity holding one of their verified emails. On each request Cairn finds that identity, creating one when none exists; a login with no verified email is refused. People are always shown by name, never an id.
- **H4** Every authenticated user can do everything in the deployment.
- **H5** Optimistic locking per domain: a patch whose base revision has moved is refused as stale, whatever it writes; the writer reads again and decides again. A journey write that names an identity is stale when the deployment has moved since the identity was chosen. Every patch carries a client-made id, and resubmitting one that committed returns its receipt.
- **H6** Every open view refreshes when its data changes, with no reload.

### I. AI / agent access

- **I1** Everything the UI does is in the API; the UI is its client.
- **I2** A hosted, remote MCP server on the same authentication, usable with no local install, covers everything the UI does.
- **I3** The snapshot is the first agent read and stays small at any size: the journey's facts and counts. Each count is the total of a node list filter, and reads of those lists and of single nodes carry the rest.
- **I4** Agent instructions ship with the server, so any connected agent gets them.
- **I5** The in-app assistant uses the same tools and instructions as an external agent. It writes what the user asks for directly, and proposes a change that is structural, large, or ambiguous. It never applies a proposal or imports a file. It is optional.
- **I6** A proposal is where a patch lands: any write naming a proposal adds to it instead of applying, creating it if new. Anyone can review, edit, apply, or discard one. Apply checks the destination's base, validates, and commits atomically; a stale proposal is refreshed and reviewed again.
- **I7** An agent has its user's capabilities; applying a proposal needs the same confirmation from anyone.

### J. Audit log

- **J1** Every mutation appends an event: actor, time, subject, and the after-state needed to replay it. Events never store derived values.
- **J2** One write path, the patch applier, writes state and events in one transaction.
- **J3** A replay harness rebuilds state from the log and compares it with stored state after every step of the scenario tests; drift fails the build.
- **J4** History is viewable per node and per journey, grouped by patch.
- **J5** Events are queryable by journey, node, user, type, patch, and time.

## Invariants

Enforced on every patch; a violating patch is rejected with every violation and nothing is stored. Derived flags (overdue, blocked, stale, unassigned, shortfall) are never violations.

- Ids are sibling-unique slugs, and every reference exists and is of the right kind. No route id is `new`.
- Containment is a tree. Dependencies, implicit ones included, are acyclic.
- Dates contain no contradictory chain.
- Conditions never read a decision in the node's own subtree.
- Estimates, offsets, and weights are non-negative.
- Keys are unique within a graph and never reused: a graph remembers every retired key.
- Answers match their decision's type; identity references resolve.
- A journey's lineage names an existing route version.
- A patch's base revision is its target's current revision.

## Non-functional requirements

- **Config-first**: an author adds a node in minutes with no code access; every validation error is fixable without reading source.
- **Small-team scale**: tens of users, hundreds of journeys, thousands of nodes per deployment; deriving a whole journey on each read feels instant.
- **Deployable**: one binary with a database, one command locally; the assistant optional.
- **Observable**: structured logs and metrics.
- **Domain-free**: code, schema, fixtures, tests, comments, shipped docs, and agent instructions never name a specific use case Cairn serves or the people or organizations behind one, even when a conversation does. Examples are invented and generic (a vendor evaluation, a hiring loop, a product launch).

## Later

Deferred: sign-off; recurring items; per-member fan-out; soft edges; a CLI; custom state machines; reminders; presence and co-editing; journey export and restore; access control; default answers (they hide that a decision is open); cross-journey priority; per-user assistant credentials; several instances on one database; patch revert; upgrading a journey or an inserted root to a newer route version.

## Success criteria

- A participant knows within a minute of opening a journey where they are, what's next and why, what needs deciding, what's theirs, what's blocked, and what's overdue.
- An author gets a reviewable route draft by describing a process to the assistant in ten minutes.
- Someone with no route gets from an empty journey to an applied structure and a ranked frontier in ten minutes.
- An agent over remote MCP starts a journey, answers its decisions, proposes a breakdown, and sees the frontier the UI shows.
