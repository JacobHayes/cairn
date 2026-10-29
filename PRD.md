# Cairn: PRD

## Summary

Cairn structures a process as one graph of **nodes** to be decided or done: decisions (typed questions), deliverables, actions, milestones, and groups. Nodes nest (containment), depend on each other (`requires`), and are filtered by answers (conditions), which also decide who is involved and when things are due. A **route** is a versioned, stateless template; a **journey** is a live graph with state, usually copied from a route version (keeping lineage for upgrades), or started empty and structured as it goes, and savable as a route. Every change is a validated, atomic **patch**. Cairn ranks the frontier by gravity, slack, and unlocks. People use a web UI (a zoomable graph, lists, a timeline, card triage); agents use a hosted HTTP API and MCP server with the same capabilities, and an in-app assistant uses the same surface. Conversation with an assistant is the expected main authoring path; the canvas and file import are the others, all producing the same patches.

## Goals

1. A process encoded once as a route, entirely as data.
2. A journey always answers: where am I, what's next, what needs deciding, what's blocked, what did I forget.
3. A ranked frontier with its trade-off visible.
4. Ad-hoc journeys with no route, structured incrementally, savable as routes.
5. Involvement explicit: decisions fill roles, nodes inherit participation, any node overrides.
6. Multiplayer, every change attributed and logged.
7. AI-native: agents can do everything the UI does, remotely with no install.
8. General: other processes and deployments need no code change.

## Non-goals (for now)

- Role-based access control: every authenticated user reads and writes everything in a deployment.
- Project management: no capacity, sprints, leveling, or auto-scheduling; feasibility is checked, never scheduled.
- Hosting artifacts (Cairn stores links) or doing the work (it provides templates and drafts).
- Custom state machines, sign-off, recurring items, per-member fan-out (`Later`).
- Automatic sync between route files and the database.
- Tolerating invalid structure: a patch that breaks an invariant is rejected.
- Learned priority: weights are authored and ranking is a documented formula.

## Users

Participants (primary), route authors, ad-hoc structurers, observers, and AI agents acting for a user. People a journey refers to are often not users: an identity is not a login.

## Core concepts

The glossary: one definition per term; behavior is specified where cited.

| Term | Definition |
|---|---|
| **Deployment** | One installation: users, identities, routes, journeys. One trust boundary. |
| **Graph** | The structure routes, segments, and journeys share: nodes, edges, roles, participation kinds, conditions, date rules, weights, resources. |
| **Route** | A named template graph: immutable numbered **versions** and at most one **draft**; no state. **Kind** `process` or `segment`, fixed at creation. Journeys, lineage, save as route, and re-link concern process routes only. |
| **Route version** | An immutable numbered snapshot; journeys are created from and upgraded to versions. |
| **Route draft** | A route's one mutable graph; publishing it creates the next version. |
| **Segment** | A route of kind `segment` with one root, inserted rather than started (A21). Avoid: template, snippet, module, sub-route. |
| **Insertion** | One placement of a segment version in a graph: key, version, parent, role and kind maps, members (B13, B14). |
| **Journey** | A live graph: nodes, answers, node state, participations, pins, snoozes, notes, links, history, owners, optional lineage. Status `active`, `completed`, `archived`. |
| **Journey owner** | An identity responsible for a journey (zero or more). Not a participation; not saved into a route. |
| **Lineage** | A journey's link to the route version it was created from or last upgraded to. |
| **Node** | The one recursive graph unit: `kind`, id, key, title, weight, kind fields (A1a). |
| **Node kind** | `decision`, `deliverable`, `action`, `milestone`, `group`. |
| **Decision** | A typed question (A4); may fill a role or pin a milestone. A leaf. |
| **Deliverable** | Work producing an output; may require an artifact; may have children. |
| **Action** | A step of work; may have children. |
| **Milestone** | A point in time with a date and `done` state, possibly externally owned. A leaf. |
| **Group** | A pure container, never actionable; state derived unless marked done or skipped. |
| **Container** | Any node with children. |
| **Parent / child** | Containment; at most one parent. |
| **Answer** | A journey's value for a decision, with optional **rationale** (B2). Empty text or list is answered; no submission is unanswered. |
| **Edge** | A hard dependency, `requires`. |
| **Condition** | `relevant_when`: a predicate over answers deciding relevance; it gates on the decisions it reads. |
| **Role** | A single- or multi-valued slot filled per journey with identities. |
| **Filling decision** | The one decision, if any, whose answer fills a role. |
| **Participation kind** | Built-in `owner` plus graph-declared kinds; informational. |
| **Participation** | A node's `{kind -> source}`, the source a role or explicit identities. |
| **Owner** | The single built-in participation; a decision's decider. |
| **Identity** | A deployment-scoped person or team, with a name, emails, or both (H3). |
| **User** | An authenticated account; agents act for a user. |
| **Login** | A provider, subject, verified emails, and optional name. A user has one or more. |
| **Pin** | An explicit journey date on a node. |
| **Date rule (`due_by`, `not_before`)** | An authored constraint relative to a milestone, date answer, or `created_at` (A8). |
| **Due** | Derived latest finish (F3). |
| **Actual date** | The recorded date a milestone was done; a fact. |
| **Effective date** | A milestone's actual, else pin, else due (F1). |
| **Weight** | Authored intrinsic value (default 1, groups 0). |
| **Gravity** | Derived weight riding on a node (Priority). |
| **Slack** | Latest start minus today (F3). |
| **Constraint** | "B is at least k days after A" (F2). |
| **Earliest start / latest start** | Derived start bounds (F3). |
| **Contradictory chain** | Plan constraints requiring a date to precede itself (F5). |
| **Shortfall** | Derived warning that the plan ahead cannot be met (F6). |
| **Unlocks** | Derived weight completing a node frees now (Priority). Not the `unlocked` consequence list. |
| **Rank** | Derived blend ordering the frontier (Priority). |
| **Frontier** | The actionable nodes; it has no actionable descendants. |
| **Acting frontier** | The frontier minus snoozed and scheduled nodes. Drives the next list, triage, snapshot counts, and `stalled`. |
| **Scheduled** | Held off the acting frontier by a date ahead (D8). |
| **Undecided** | Relevance reading an open, relevant decision; displayed conditional. |
| **Stalled** | Empty acting frontier with in-scope unfinished nodes (D5). |
| **Actionable** | Relevant, not blocked, non-terminal (D2). |
| **Blocked** | Off the frontier on an unsatisfied dependency; never stops a move (Gating). |
| **Display state** | The one derived status every surface shows (D8). |
| **Terminal** | `done`, `skipped`, `decided`; satisfies dependencies (D1a aside). |
| **Guard** | A completion precondition on evidence, bypassable (D4). |
| **Stale** | Finished work to revisit (D4). |
| **Snooze** | "Not now" until a date or a node (B6). |
| **Skip** | Terminal "not doing this", reason required; cascades on containers (D1a). |
| **Placeholder** | Work each journey must break down or mark `atomic` (B10). |
| **Condition gate / implicit edge** | A dependency derived from a condition, containment, inherited requirement, or stage opening; behaves like `requires`. |
| **Touched set** | Everything a patch or event writes or removes, by key and field, including removed subtrees and incident edges (H5). |
| **Upgrade available** | A newer published version of a journey's lineage route or an insertion's segment. Retiring hides a route only from new journeys. |
| **Structural change** | A change to what the graph is: nodes, edges, roles, kinds, conditions, date rules, stage bounds, resources; route, upgrade, re-link, and identity-merge operations. All else (transitions, answers, participations, weight overrides, pins, actuals, snoozes, notes, links, overrides) is state. Every route change is structural. |
| **Consequences** | What a patch newly caused in derived state (D7). |
| **Override** | Force include or keep, with a reason, logged. |
| **Keep** | Exempts a subtree from an inherited skip (D1a). |
| **Local edit** | A per-field marker that a journey changed a route-copied field (B4). |
| **Provenance** | `from_route`, `from_segment`, `local`, or `orphaned`. |
| **Node tombstone** | A record that a route-copied node was removed, so upgrades skip it. |
| **Patch** | Ordered mutations applied atomically against a base revision; the only write. |
| **Mutation** | One change in a patch; one event. |
| **Revision** | A monotonic number per domain (route with draft, journey, deployment) and per proposal. |
| **Proposal** | A patch drafted, not applied. |
| **Resource** | Route guidance on a node: tip, template, example, reference, message draft. |
| **Note / Link** | Journey annotation: note, artifact, reference, conversation. |
| **Attachment** | The model behind both: content with a scope and a type. |
| **Artifact** | A link designated as a deliverable's output (G2). |
| **Checklist** | Ordered tickable items on a deliverable or action (G5). |
| **Required note** | A note needed before `done` (G4). |
| **Notice** | An advisory finding on a route graph (A20). |
| **Event** | An append-only record of one mutation. |

### The three relationships

Containment (a tree), dependency (`requires`, a DAG between non-ancestor, non-descendant nodes), and relevance (conditions) are independent, except that a parent requires its relevant or undecided children and children inherit their parent's dependencies (Containment).

### How the pieces fit

Route versions, drafts, and journeys share one graph structure (a segment is a route). A journey adds state keyed by node: stored state, provenance, local edits, `atomic`, answers, role fills, pins, snoozes, notes, links, overrides, tombstones, events. None exists on a route. A proposal is not journey state: it names its destination domain (possibly not yet existing) and has its own revision.

### Keys and references

- Nodes, roles, and participation kinds have an **id** (a slug unique among siblings: children of one parent or the roots; roles and kinds within the graph) and a stable opaque **key** (`n_7q4m2k`), assigned once at creation and kept through renames, moves, copies into journeys, and exports. A node's **path** is the slash-joined ids from the root (`testing/report`), without the kind.
- Inserting a segment is the one copy that mints keys: each derived from the insertion key and the segment key, so a segment can be inserted twice and an upgrade's new node gets the key the same derivation gives (B13, B14).
- **References are stored by key**; paths and ids are for files and display. Edges, conditions, date rules, stage bounds, snooze targets, `fills_role`, `feeds_milestone`, participation sources, and message-draft placeholders resolve to keys at import or edit, so renames and moves break nothing. Journey state and lineage match by key.
- Files carry paths, ids, keys, the route id, and the base version. On import a keyless node matches by path against the base version and inherits its key, else gets a new one; keys are unique in the file; a key absent from the base adds a node. An occurrence is `(graph, key)`; other graphs may share the key. Export emits current paths, ids, and keys.

### Containment: what the tree means

- **A parent implicitly requires each relevant or undecided child** (D1a's kept-work exception aside). A relevant unfinished deliverable or action reaches the frontier for its own completion once its children and dependencies are satisfied; until then its actionable children are the frontier. Completing a parent first leaves its children open.
- **Children inherit their parent's dependencies**: an explicit `requires` into a node gates every descendant. A stage's opening is one: the stage requires its `opens_at` milestone (unless `gates: false`).
- An edge into a parent waits for the parent's own completion or skip, not just its children's.
- Through these edges a child's gravity includes its parent and the parent's downstream, and its due is bounded by the parent's latest start. A parent's estimate is its own work after its children.
- Implicit edges (containment, inherited requirements, condition gates, stage openings) are derived on read, count in blocking, trace, gravity, and cycle checks like explicit ones, and are named by source (parent, ancestor's requirement, decision, stage) in explanations and violations. A group satisfies dependents when its relevant or undecided children and own dependencies are satisfied; an empty group still waits for its gates.

### Gating: how decisions sit in the graph

- An ungated decision is actionable at once (the up-front pass); a gated one (`requires`) once its prerequisites are done, so roles and dates can be settled late. A personal "later" is a snooze.
- A node that `requires` a decision or reads it in `relevant_when` waits for its answer. An explicit `requires` duplicating a condition reference is rejected.
- **Relevance** is `relevant`, `not_relevant`, or `undecided`, combining a node's condition with its ancestors' (`not_relevant` beats `undecided` beats `relevant`); a node with no condition inherits its parent's. Node detail names the source.
- Conditions are Kleene three-valued. Only an `open`, relevant decision gives `undecided`, so no node stays undecided on a decision nobody will make. A `skipped`, `not_relevant`, or undecided decision is **unanswered**: `is answered` and the value operators (`equals`, `not equals`, `in`, `contains`) are false, and `not`/`and`/`or` compose normally, so `not (x equals yes)` models the other branch when `x` is skipped.
- **Blocked**: a relevant or undecided hard dependency of any source is unsatisfied (an undecided one blocks until its decision rules it out). Blocked nodes leave the next list and triage but count for gravity and show in traces. Blocking never stops a move: start, complete, answer, annotate, assign, pin, snooze, reweight, and break down all stay open, and work done ahead reads like any other (D4, F6).
- **Force include** (with a reason) makes a node relevant despite its own or ancestors' conditions or unanswered decisions, dropping those conditions' implicit gates; explicit `requires` still apply, and descendants still evaluate their own conditions. There is no force skip: skip the node.
- **The frontier**: a parent with an actionable child is blocked on it, so only the child appears; groups never do. Acting surfaces (the next list, triage, the `next_up` filter) show each item with its ancestors.

### Priority: weight, gravity, slack, unlocks

The formulas are normative; the constants are per-deployment-configurable defaults, always displayed.

- **Gravity**: `effective_weight(d) = weight(d) * (0.5 if d is undecided else 1)`, `gravity(n) = effective_weight(n) + sum over d in downstream(n) of effective_weight(d)`. `downstream(n)` is every transitive dependent through all gates and containment, each once, pruning not-relevant branches, and skipping terminal nodes' weight while traversing them to unfinished dependents. A container also displays **subtree gravity**: the same sum over `S` and `downstream(S)`, `S` being it and its open in-scope descendants, each node once (0 when all done); its own `gravity` still orders it.
- **Slack** = `latest_start - today`; null when unbounded ("no deadline").
- **Unlocks**: `unlocks(n) = sum over d in immediate_unblock(n) of weight(d) * owner_factor(d) * (0.5 if d undecided else 1)`, over the in-scope, unfinished, non-group nodes whose remaining hard dependencies completing `n` would satisfy (including through derived group completion), relevance held fixed, each once. `owner_factor` is 2 when `d`'s owner differs from `n`'s (or `d` is unowned and `n` owned), else 1. Shown with its split ("unlocks 3 tasks, 2 owned by others").
- **Rank**: `urgency = clamp((14 - slack_days) / 14, 0, 1)`, `late = clamp(-slack_days / 14, 0, 1)`, null slack giving 0 for both; `gravity_norm` and `unlocks_norm` divide by their maximum over relevant or undecided, unfinished, non-group nodes in the journey (0 if that is 0); `rank = 0.40 * urgency + 0.15 * late + 0.25 * gravity_norm + 0.20 * unlocks_norm`. Ties: smaller slack (null last), greater gravity, key. `overdue` and the due colour read `due`; urgency and slack read `latest_start`. Rank is global (owner factor relative to the node's owner); a "rank for me" sort recomputes it for the viewer without changing it. Any single signal can be sorted on, and an effort-adjusted sort (gravity per estimated day, missing or zero estimates last) is offered when estimates exist.
- Node detail shows every signal with its inputs (contributing nodes, the pin behind the slack, the nodes it would unlock and their owners).

## Functional requirements

Deferred items are under `Later`.

### A. Graph definition (routes, segments, and journeys share this)

- **A1** A graph is entirely data in a schema-validated document; no content change needs a code change.
- **A1a** Node fields (kind; default): `key` (all; minted once), `id` (all; sibling-unique slug), `kind`, `title`, `description` (markdown, optional), `weight` (all; 1, group 0), `estimate` (deliverable/action; optional, days), `requires_artifact` (deliverable; false), `requires_note`, `requires_checklist`, `placeholder` (deliverable/action; false), `checklist` (deliverable/action; ordered `id`/`title` items, at most 50), `relevant_when`, `due_by`, `not_before` (all; optional), `fills_role` (identity or identity-list decision), `feeds_milestone` (date decision), `final` (milestone; at most one per graph), `auto_done` (milestone; false), stage bounds `opens_at`/`closes_at`/`gates`/`closes` (group), decision-only `prompt`/`answer_type`/`choices`/`help`, journey-only `atomic` (placeholder). Groups take no estimate.
- **A2** Containment is a tree of any depth (convention group > deliverable > action). Groups, deliverables, and actions may hold any kind; decisions and milestones are leaves.
- **A3** `requires` joins any two nodes regardless of kind or depth, except a node and its ancestor or descendant.
- **A4** A decision has a prompt, an answer type (boolean, single choice, multi choice, text, date, identity, identity list), optional choices, and optional help. It may require nodes and be read by any node's conditions. It has one owner; a panel decision names a facilitator as owner and the panel through another kind.
- **A5** `relevant_when` reads decisions with a fixed operator set (equals, not equals, in, contains, is answered, and/or/not), three-valued (Gating), and applies to descendants.
- **A6** Roles are declared single or multi. An identity-list decision fills only a multi role, a single-identity decision only a single role; a role has at most one filling decision. Nodes declare default participations `{kind -> role}`. A graph may name a `default_owner` role, used where no ancestor supplies an owner.
- **A7** Beyond built-in `owner`, a graph declares kinds, each with a `multi` flag and stable key; they are informational and single-valued ones cannot take multi-valued roles.
- **A8** Date rule `due_by: {before|after: <source(s)>, offset: <days>}` (offset default 0); sources are milestones, date-decision answers, and the journey's `created_at`, each one constraint. Decisions may carry decide-by rules. `not_before` has the same shape and gives an earliest start (F2). A deliverable's, action's, or decision's own `not_before` date (the latest its rules give) holds it scheduled while ahead (D8); acting earlier is a plain move, never guarded and never a shortfall. A container's date holds only its own work.
- **A9** Dates, estimates, and offsets are calendar days; "today" is in the deployment's configured time zone.
- **A10** Resources on any node: markdown tips, typed links, and message drafts with placeholders (`{{journey.name}}`, `{{roles.eval_owner.name}}`, `{{answers.testing/scope}}`; journey fields `name`, `description`, `created_at`, `url`, `status`). An unfilled role or unanswered decision renders a visible marker; rendering never fails.
- **A11** Routes have immutable numbered versions and at most one draft, which extends the latest version and is what patches target; publishing creates the next version and clears it. Opening a draft (patching a version, importing an extension, saving a journey as a route) while one exists is rejected, offering to discard it. Route edits reach a journey only through a confirmed upgrade.
- **A12** Routes are authored in conversation (assistant or external agent, reviewed as proposals), on the canvas and node detail, or by file import, all as patches against the draft, and all export the same file.
- **A13** Routes and segments (`kind: segment`) import and export a canonical file, round-trip safe and tested: a version or the draft out; a new route or a new draft extending the latest version in. A file carries structure, not lineage: an insertion's nodes are written and read as ordinary nodes.
- **A14** Graphs are stored as structured fields, relations, and constraints; the file serializes them.
- **A15** A patch that would break an invariant is rejected with every violation by path; no invalid graph is stored. Notices (A20) are the only advisory findings.
- **A16** Completion is gated only by `requires_artifact`, `requires_note`, `requires_checklist`, `placeholder`, and the implicit child requirement.
- **A17** A patch targets one domain (a journey, a route with its draft, or the deployment) or a proposal in it, names the base revision, and holds ordered mutations. State-machine legality is checked at each mutation's position; invariants once, on the resulting graph, against which derived-state guards are also checked (D4), so no intermediate graph is derived and a bulk completion of a chain is accepted in any order. All mutations and events commit in one transaction or none. Creating a journey or route is its first patch, base 0, producing 1. Identity edits and merges target the deployment; an identity create may ride in any patch (E6). Otherwise multi-domain operations are separate patches, except hard deletion (A19). Applying a proposal commits its mutations and applied status atomically (I6).
- **A18** Removing a node removes its subtree and incident edges. Any other remaining reference to it (condition, date rule, stage bound, snooze, `fills_role`, `feeds_milestone`) invalidates the patch unless rewritten or removed in it; the UI and assistant build that cascade. A removal names everything it removes (subtree, edges, attached notes, links, artifacts, resources, participations); if at apply any of it holds something unnamed, it is rejected, so a stale removal never widens. A role or kind is removable only when nothing references it.
- **A19** A draft can be discarded; a route or segment retired (hidden from new journeys or insertions); an identity deleted only when unreferenced (E6). A journey can be archived (B11) and hard-deleted with its proposals and events. A route or segment can be hard-deleted with its draft, versions, proposals, and events only while unused (no journey follows it, and no journey, draft, or version holds an insertion of it); a version is never deleted alone. A hard delete is one deployment patch naming each journey and route with its read revision, all or none; it is stale if any moved or a route came into use, rechecked at commit. The deployment log records who deleted what and when, and deleted ids are never reused.
- **A20** Notices are reported on route import, publish, and while authoring; never stored, never rejecting. The one notice: in a graph with a `final` milestone, every non-group node with no chain to or from it, through dependencies (implicit included) and date constraints, with every condition treated as relevant. Decisions that fill roles are exempt. Segments and journeys are not checked.
- **A21** A segment is versioned, drafted, retired, imported, and authored as a route. It declares no `final` and no `default_owner`, has at most one root (a version exactly one), and references only its own nodes, roles, kinds, and `created_at`. It is never started or a lineage; it is inserted (B13).

### B. Starting and shaping a journey

- **B1** Create a journey from a route version (default the latest) or empty, with a name, optional description, and owners (none by default, not even the creator). From a route, nodes keep their keys and lineage is recorded. Initial states (create, copy, breakdown): deliverable/action `todo`, decision `open`, milestone `pending`, group derived. Name, description, and owners are edited together while not archived; upgrades keep owners.
- **B2** Decisions can be answered when actionable, ahead of their dependencies or date, or while undecided (with D4's warning; the answer counts once relevant). Answers are revisable, logged, and re-evaluate the graph; state on nodes that become not relevant is kept. A rationale (markdown) belongs to its answer: a revision without one leaves none, a rationale-only change is a revision, history keeps both, reopening clears both.
- **B3** Filling a role picks existing identities or creates new ones (name only is enough). Identities need not be users.
- **B4** Journeys may add, edit, move, and remove nodes, edges, roles, conditions, weights, and resources. Copied nodes are `from_route`, new ones `local`. Editing a route-copied field, edge, participation, condition, rule, or resource sets a per-field local-edit marker; "reset to route" clears it. Removing a route-copied node tombstones its subtree.
- **B5** Participations, pins, and weights are ordinary journey state; force include and keep are overrides (reason required, logged); completing past a guard is logged on the completion (D4). All survive role and answer changes and upgrades.
- **B6** Snooze an actionable node, or a container with unfinished in-scope descendants, until a date or until one named node completes. A node target that is the snoozed node, in its subtree, or depends on either is rejected. A container's snooze holds over its subtree, each descendant naming it; a descendant's own snooze is independent, and unsnoozing one held only by its container is refused, naming the container. Holding is derived on read: a date snooze while today is before it; a node snooze while its target neither satisfies dependencies nor is not relevant (so it holds again if the target reopens or an `auto_done` date moves back). Any transition on the snoozed node itself clears it, in that transition's event. Snoozed nodes still block and count for gravity but leave the next list and triage. An explicit unsnooze is always available. Snoozes are logged.
- **B7** Upgrade: diff the journey's version against the target by key (nodes, edges, roles, kinds, conditions, rules, resources) and merge per field: unedited takes the target; locally edited and unchanged in the target keeps the journey's (listed); edited on both sides is a conflict. New nodes are added unless tombstoned. Removed nodes become `orphaned`, keeping parent, edges, and state, each keep-or-remove (default keep; removing cascades to local descendants), along with the roles, kinds, and resources they reference (keep, remap, or remove; one an insertion maps onto only keep or remove, and removing a role gives the segment's nodes its fillers directly). Schema and state conflicts (removed choice, answer type, role cardinality, kind) resolve by mapping old to new, keeping the local definition, accepting the target and clearing invalid state, or reopening. The result is a proposal applied on confirmation; violations are unresolved conflicts.
- **B8** Save as route: the journey's structure (no state, pins, answers, notes, or owners) becomes a new route draft or a new draft of its lineage route, keys preserved. Review maps each explicit identity (drop, to a role, new role, `default_owner`) and can exclude nodes; breakdown children default to excluded.
- **B9** Re-link: after publishing a save-as-route draft, a reviewed proposal links the journey to the new version. Included nodes match by key; markers clear only on matching fields, differing values stay local edits unless the reviewer takes the route's; excluded nodes stay local. Lineage and markers change atomically at apply.
- **B10** Breakdown: deliverables and actions gain children by patch, by hand or by proposal, including "one child per role member", which writes explicit participations and flags a child whose identity leaves the role; other children inherit live (E2). A placeholder "needs breakdown" until it has children or is marked `atomic` (journey state, survives upgrades).
- **B11** Status: completing is the user's call, suggested when the journey has a node and every relevant or undecided node satisfies dependencies, or the `final` milestone is done. A completed journey stays patchable, can reopen, and leaves "mine" and cross-journey lists. An archived journey accepts only a move back to `active` or `completed`; deletion is the deployment's (A19).
- **B12** Reserved.
- **B13** Insert a segment: a patch to a journey or route draft puts a published, unretired segment version under a group, deliverable, action, or the top level, with a client-minted insertion key. It copies nodes, edges, conditions, rules, participations, and resources, each key derived from the insertion key and segment key (a key the graph holds or retired rejects it). The root takes the segment root's id (or the smallest free suffix, `review-2`) and title unless given others. Each segment role and kind maps to the graph's of equal cardinality or is added; unmapped, it takes the graph's of the same id and cardinality, else is added, suffixed if needed. Segment nodes may be left out with their subtrees (so mapping onto an already-filled role leaves out the segment's filling decision). The same mutation may add edges to graph nodes. In a journey, members are `from_segment` and start in initial states. A journey created from a route version holding insertions sees `from_route` nodes and no insertion; the route's insertion is upgraded instead and arrives by route upgrade. A segment's own insertions are not recorded where it is inserted. Removing the last member removes the insertion; removing a role or kind drops the maps onto it. From an agent it arrives as a proposal.
- **B14** Insertion upgrade, by proposal: an insertion keeps no markers or tombstones; its local edits are its members' differences from its version (ignoring the root's parent, id, title, and edges to non-members). Diff by segment key and merge as B7 (new nodes under derived keys unless their parent was removed; removed ones orphaned and leaving the insertion). Mapped roles and kinds never change; new ones map or are added as in B13. A route upgrade never changes a journey's `from_segment` nodes.
- **B15** Save as segment: selected nodes and descendants become a new segment draft with B8's review (no default owner), keys preserved; one top-most node becomes the root, several go under a new group root. Nothing outside is carried, and each cut is listed: crossing edges dropped; outside condition decisions cleared; outside rule sources dropped (the rule with its last); outside stage bounds and fed milestones cleared; outside answer placeholders become the decision's title; inherited outside participations written onto the root; `final` dropped. Once published, the source nodes (top-most sharing a parent, wrapped in a root if several) can be linked to that version by proposal, becoming an insertion; a linked `from_route` node leaves its route's lineage.

### C. Graph, navigation, and views

What a person can see and do; how it looks is DESIGN's.

- **C1** Canvas: a journey's graph with pan and zoom. A card shows kind, title, display state, its most actionable date and, where it matters, its owner; a decided decision shows its answer. Edges show direction and are told apart by kind (explicit, condition gate, stage opening); hovering one says what it is and whether it still waits. Date relationships are not edges: only the selected node's are drawn, while its dates are open. Not-relevant nodes are hidden and conditional nodes ghosted by default, each toggleable; the Filter always counts what its settings hide (not-relevant apart), so hiding never hides a problem.
- **C2** Semantic zoom. Depth is a ladder: Stages (top-level groups and milestones), Decisions (adds decisions, nested groups and milestones), Deliverables (adds deliverables, their actions rolled up), Actions (all). With no top-level group there is no Stages step. Containers also collapse and expand singly. Depth, the Filter, and search are one view state shared by every view of a journey or route and carried across switches; a view that cannot honour a setting keeps it, disabled with the reason. They choose what is drawn; Frontier and Fade only emphasise. Roll-up rules are fixed. A node is visible when its kind is in the step (at Stages, its top-level ancestor is a group or milestone), its display state is shown, the Filter and search keep it or a descendant, and no ancestor is collapsed. The selected node is always visible with its containers expanded, without rewriting any setting; collapsing a container holding it selects the container. Hidden nodes roll up into their nearest visible ancestor, and edges to them are drawn to it (deduplicated, omitted when none exists or both ends land on one node); a visible node with a hidden unsatisfied prerequisite and no visible stand-in is marked and the mark opens the trace, so hiding never makes blocked work look free. A container shows its progress (done of in scope over everything beneath it, counted by the engine), whether children are active, all blocked, need a decision, or need breakdown, its subtree gravity, its children's minimum slack, and their owners.
- **C3** At Actions with every display state shown, the whole graph is in one canvas.
- **C4** Drilling in and out expands and collapses containers in place.
- **C5** "I am here": the frontier and `active` nodes stand out, and the viewer's own items say so. At Stages the current stages (holding an acting-frontier or active node) open by default; the graph opens at Actions fitted to the whole graph. The top-ranked few carry rank tags. With an empty acting frontier the stalled diagnostic (D5) shows.
- **C6** Priority is visible but quiet: the due date has an urgency colour with words; latest start and slack are shown apart; a signals lens labels each card with one signal (rank, gravity, unlocks, slack). No border weight or opacity encodes a signal; nothing animates.
- **C7** Trace: selecting a node marks, in words, everything upstream (transitive dependencies, gating decisions, stage openings) and downstream (dependents, ancestors, nodes whose relevance it decides), across levels, terminal and not-relevant included, with gravity contributors marked. A skipped container or group marked done waits only on its pending kept work. Traced nodes the view hides are counted and revealable. An edge can be selected, with a way to each end.
- **C8** Node detail: display state, with stored state when different, and what it means; description; resources, notes, links; for a decision the prompt, editable answer, rationale, history, and each choice's effect (nodes brought in, dropped, or left undecided; pin set; role filled); children with progress; checklist, tickable in place; participations; dates with chains (F7); signals with contributors (unlocks split by owner; dependents it would not yet free and what else each waits on); relevance with its source, pending included; derived flags (`blocked`, `unassigned`, `stale` with reasons, `overdue`, `snoozed` with the day it lifts, `needs_breakdown`); snooze; and last, folded, provenance, a member's insertion and local edits, and history as sentences naming people (never ids).
- **C9** List view: the drawn nodes (C2) as a table, a tree by container, flat when sorted. At Decisions, each decision's answer, rationale, and affected nodes are columns. Filter: only mine, one owner, overdue, stale, unassigned, shortfall, snoozed, not relevant, conditional. Search over title, description, notes, and resources. Sort by rank, slack, gravity, unlocks, or due. Bulk transition, assign, snooze, and skip on a selection: one patch, one event per node; any guard failure fails the patch and names the node.
- **C10** Frontier: a highlight on every view (the acting frontier lit, the rest faded, nothing hidden), not a page. On the list each row gives one reason in words (why it ranks there, or the date that matters) and its primary action. Only mine narrows to the viewer's items; unassigned items offer assignment inline. It matches the agent's next-up list (I3).
- **C11** Triage (the walkthrough): the acting frontier one card at a time in rank order; a card is the node's detail. `pass` reorders for this pass only, client-side, no event. Per kind:
  - `decision`: answer, skip (reason), snooze
  - `deliverable` / `action`: start, done (guards, with an inline artifact link or note when required), snooze, skip (reason), break down (a proposal), open on canvas
  - `milestone`: mark done, skip (reason), snooze, open on canvas
  - placeholder: break down, mark atomic, snooze; no done

  No group cards. The view state narrows the cards. At Decisions depth it is the **decision walkthrough**: a new journey opens on it, it can be launched any time, and with nothing actionable it shows what would unblock the earliest decisions. Triage always reads the current frontier; after an action, the nodes it brought onto the acting frontier (D7) come next, in rank order, labeled with the card that unlocked them. Pass order is client state; the shared rank is unchanged.
- **C12** Decision view (Decisions depth): decisions, their groups, milestones, and gating edges; selecting a decision traces what it affects. A projection, not a structure.
- **C13** Timeline: dated nodes the view state draws, on a time axis following the tree (not-relevant work has no dates), overdue and shortfall marked, the `final` milestone as end anchor. A route's timeline is relative: the route as a journey starting day 0, each node at its earliest allowed day, the final milestone marked conditional when it passes an undecided branch; a rule waiting on an unanswered date decision says so. Undated nodes are listed apart; with no dated final milestone the axis says there is no end anchor.
- **C14** Proposal review: a proposal is a layer over its destination's own views (a journey's or route's graph, list, timeline; the deployment's identities), changes marked in place, with an item-by-item editable list (nodes added, changed, removed with cascades; edges, pins, answers, conflicts, orphan choices, participation mappings), then applied or discarded. A proposal may hold unresolved conflicts and dangling edits; validation and guards run on the final candidate (A17, D4), strictly at apply. It has one destination (a journey, a route or its draft, or the deployment), possibly not yet existing, and is listed under it while open; one creating a journey or route is listed on that index. Its only addresses: `/journeys/<id>/proposals/<proposal>`, `/routes/<id>/proposals/<proposal>`, `/identities/proposals/<proposal>`, plus `/nodes/<key>` below the first two. With the layer on, the page offers no live write. Apply is enabled once nothing holds it (an unresolved choice, a stale destination, unsaved edits, a rule-breaking candidate) and is sent at the proposal revision shown.
- **C15** Layout is automatic (layered DAG) and deterministic; small edits move few nodes.
- **C16** Journeys index: filter by status, lineage route, version, owner, "mine" (active journeys the viewer owns or works in), upgrade available; rows name owners; archived journeys can be deleted together with one confirmation. With nothing selected a journey shows its overview (status, owners, progress, what's next, upcoming milestones, open decisions), whose full page is the Summary (C18). A journey offers upgrade, save as route, re-link, and status moves (complete, reopen, archive, unarchive, delete once archived), a suggested completion, and a non-active status beside its title. Home (the app root, no redirect) lists, per active journey, the viewer's work that can move now in rank order with the rest of "mine" counted, and the proposals waiting on them (theirs, and agent-drafted ones for active journeys naming their identities). Every journey, node, and edge has one address (`/journeys/<id>`, `.../nodes/<key>`, `.../edges/<from>~<to>`; `.../proposals` for its open proposals), the view in the query.
- **C17** Route page `/routes/<id>`: the journey views and view state (no Only mine, no Frontier), showing the draft if open, else the latest version; a version selector picks any, kept in the address. The draft is editable there; versions are read-only. It lists journeys on the shown version, which can upgrade, and how many can reach the latest. Upgrades are one journey at a time. Open proposals at `/routes/<id>/proposals`. An unused route can be deleted with one confirmation (A19).
- **C18** Journey summary (front page and report): what is left in scope with counts by display state; the final (or next) milestone with its date and shortfall; overdue, short, and stale work with reasons; the stalled diagnostic; what waits on the viewer; decisions to make now; the rest of the acting frontier; decisions still waiting and on what; work under way; blocked work and what it waits for; work waiting for its own date; upcoming milestones; for a closed journey, what it left unfinished; recent finishes; owners, description, notes, lineage. Each list reads whole in its full page.
- **C19** Segments are listed with routes (a type filter); a segment's page is a route's, listing every insertion on the shown version and which can upgrade. Insert a segment from a journey, container, or draft: choose, place (what it follows and precedes), map roles and kinds, then review as a proposal with its nodes outlined as one insertion. Node detail shows a member's segment, version, and whether a newer exists. Upgrade an insertion from node or segment detail; save as segment from a canvas selection.
- **C20** A guided tour, offered once on Home and again on request, on a practice journey made for the person (answer a decision, see what became ready, finish work, see where a date comes from, and what the assistant is for where offered), deleted at the end, so their work is untouched. An unfinished tour is offered again; a finished or declined one only on request. It reads the engine's derived state, so any answer is a valid path.

### D. Node state and lifecycle

- **D1** Non-group nodes have a stored state from a fixed per-kind machine:

  | Kind | From | To | Transition | Guards |
  |---|---|---|---|---|
  | deliverable / action | `todo` | `active` | start | relevant or undecided |
  | | `active` | `todo` | stop | none |
  | | `todo`, `active` | `done` | complete | in scope (undecided warns), `has_artifact`, `has_note`, `all_ticked`, `broken_down` |
  | | `todo`, `active` | `skipped` | skip | reason; cascades (D1a) |
  | | `done`, `skipped` | `todo` | reopen | none |
  | decision | `open` | `decided` | answer | in scope (undecided warns); answer matches type (an invariant) |
  | | `decided` | `decided` | revise | none |
  | | `open` | `skipped` | skip | reason |
  | | `decided`, `skipped` | `open` | reopen | clears answer, rationale, and the role fill or pin it produced |
  | milestone | `pending` | `done` | complete | in scope (undecided warns); sets the actual date (default today, editable) |
  | | `pending` | `skipped` | skip | reason |
  | | `done`, `skipped` | `pending` | reopen | clears the actual date |
  | group | (derived) | `skipped` | skip | reason; descendants inherit skipped |
  | | (derived) | `done` | mark done | reason; unfinished descendants inherit skipped (D1a) |
  | | `done`, `skipped` | (derived) | reopen | none |

  A group is otherwise done when its relevant or undecided children and its own dependencies are satisfied (an empty group still waits for its gates). Deliverables and actions have no mark done; completing one leaves its children open. Reopening is always allowed and logged.

- **D1a** Skipping a container, or marking a group done, gives its unfinished in-scope descendants an effective skip, stored states untouched; it follows containment only (skipping optional work can free a dependent without skipping it). `keep` (reason, logged) exempts a node and its subtree from inherited skips; its own relevance, dependencies, and skips still apply. Such a container (and effectively skipped containers under it) satisfies dependents only once all kept in-scope work beneath it does ("kept work pending" until then). One event records the skip or marking; reopening restores inherited-skipped descendants and keeps their own skips and keeps.
- **D2** Actionable: relevant, not blocked, non-terminal (`open`, `todo`/`active`, `pending`; never a group). Ownership does not matter (unowned is flagged `unassigned`). Snoozed and scheduled nodes stay on the frontier and leave the acting frontier.
- **D3** Derived on read, never stored: `relevance`, `blocked`, `actionable`, `unassigned`, `stale`, `overdue` (unfinished, `due` before today, by how many days), `progress`, `failing_guards`, `waits_on`, `due`, `earliest_start`, `latest_start`, `slack`, `shortfall`, `gravity`, `subtree_gravity`, `unlocks`, `rank`, `snoozed`, `needs_breakdown`, `checklist_count`, `moves` (the moves a one-move patch would be accepted for), `display_state`, and per journey `stalled`, `current_stages`, `final_milestone`.
- **D4** Guards check a completion's evidence only when a patch attempts it, against the graph that patch produces (completing and removing the evidence or relevance in one patch is rejected); they are never standing invariants, so later changes produce `stale` instead. Order is never a guard: no start, completion, or answer waits on a dependency or date. Guards, deliverables and actions only: `has_artifact` (`requires_artifact`), `has_note` (G4), `all_ticked` (one failure per unticked item, G5), `broken_down` (a placeholder has children or is `atomic`). A not-relevant node cannot complete (force include it); an undecided one completes with a warning naming the decisions its relevance reads, stays undecided, and if ruled out later is finished work that does not apply. `complete_anyway` (optional reason) bypasses the failing guards and records them until reopened, so a different later failure still makes it `stale`. `stale` (never on skipped nodes or groups marked done), with reasons: a dependency done when the node finished has been reopened and is open again (a reopen records the finish it undid; refinishing clears it; order is the journey's recorded order, so work finished after a reopen, even the same day, did not rely on it); or its guards now fail in ways its bypass did not record (missing artifact or note, unticked item, unexpanded placeholder).
- **D5** `stalled`: the acting frontier is empty but in-scope unfinished nodes remain. The diagnostic names what it waits on (a gating node, a snooze and its target or container, a scheduled node with its date and whether it completes itself) and offers what is allowed (unsnooze, mark a milestone done, start work early); it says "blocked" only when every in-scope unfinished node is blocked.
- **D6** Derived values are recomputed from stored state on every read; any cache is invisible.
- **D7** Consequences: every accepted patch and proposal preview reports what it newly caused, with explanations, computed for the response and never stored. Warnings: newly `stale` (or new stale reasons), new or larger shortfalls, newly `overdue`, finished work newly undecided, the journey newly `stalled`. Information: nodes newly on the acting frontier (`unlocked`), newly not relevant (`out_of_scope`), newly relevant or undecided (`into_scope`). Both sides use the same today. The UI shows the warnings at the edit; agents get both.
- **D8** Display state: the status every surface, the API, snapshot counts, and MCP show; stored state is secondary and what transitions act on. It replaces no D3 flag. First match wins:

  | Value | When |
  |---|---|
  | `not_relevant` | relevance `not_relevant`, not pending |
  | `skipped` | stored or effectively skipped |
  | `done` | stored `done` or `decided`; an `auto_done` milestone done by its date (F1); a group satisfying dependencies |
  | `snoozed` | its own or a container's snooze holds |
  | `active` | stored `active`, or a container waiting only on its children with work started beneath it (a deliverable or action `active` or `done`; answered decisions and done milestones are gates, not work) |
  | `conditional` | unfinished, and undecided or pending |
  | `blocked` | in scope, unfinished, an own or inherited gate unsatisfied (never a container's own children) |
  | `scheduled` | unstarted, unblocked, held by a date ahead: an `auto_done` milestone's effective date; another milestone's pin or fed answer (never its derived due); a deliverable's, action's, or decision's own `not_before` date. Never a group |
  | `ready` | everything else |

  **Pending:** a not-relevant node whose value would be `undecided` if the undecided decisions its (or its ancestors') conditions read were taken as not yet known rather than unanswered; it displays conditional naming the decision, and its relevance, priority, blocking, and guards are unchanged. Secondary detail: recorded state and cause (not relevant); reason and "kept work pending" (skipped, group marked done); stale reasons and "may not apply" (done); target and holding container (snoozed); condition and pending decision (conditional); first blocker, dated if scheduled (blocked); the date (scheduled).

### E. Roles and participation

- **E1** `owner` is the only built-in kind: one identity, a decision's decider. In-scope unfinished non-group nodes with no owner are `unassigned`, and acting surfaces offer inline assignment.
- **E2** Effective participation: explicit identities on the node, else its role reference, else the nearest ancestor's effective value (live), else `default_owner` (owner only), else none. Multi-valued kinds replace at the nearest declaring level, never union. An explicit empty value means none and stops inheritance; removing it restores inheritance.
- **E3** One authority per role: a role with a filling decision is filled only through it ("fill directly" answers it); reopening, skipping, or the decision leaving scope empties the role, and its return restores it. A role with none is filled directly. A `feeds_milestone` pin likewise: one feeding decision, and pin edits go through it.
- **E4** "Mine": nodes where the viewer's identity holds any participation, filterable by kind; teams are not modeled. An active journey is the viewer's when they own or participate in it.
- **E5** Changing a role's identities re-derives inherited participations and keeps explicit ones.
- **E6** Identity create, edit (emails included), merge, and delete are deployment patches. A merge keeps one survivor and the merged keys as aliases to it (resolving to one identity, acyclic); reads resolve aliases without rewriting journeys or history. Because conditions can compare identities, a merge is derived against every journey referencing its identities and rejected naming each that would break an invariant; it names those journeys' revisions and rechecks the referencing set at commit (a change meanwhile makes it stale). Routes need no check. A journey patch writing an identity reference names the deployment revision it was validated against (H5). Creating an identity needs no deployment revision, so one journey patch can create and reference one. A create whose key is an identity or alias is rejected, except the same patch resubmitted (H5). Delete only an identity nothing refers to directly or by alias (an answer, role fill, participation, owners, or identity condition in any journey, draft, or version; text spelling a key is no reference); otherwise it is rejected naming the referrers (up to the rejection chain limit, with a count of the rest), rechecked at commit; merge it instead. Deleting removes its emails and aliases to it. What refers to an identity is readable across journeys and routes (owners and aliases included). The Identities page searches, adds, shows what refers to a selected identity (delete enabled once that read finds nothing), and merges or deletes a selection. External milestone owners are identities.

### F. Dates, milestones, and stages

- **F1** A milestone has dependencies, an optional pin, an actual date once done, an owner (possibly external), and `done`. **Effective date**: actual, else pin, else due. A pending relevant `auto_done` milestone is done, derived with no event, when its effective date is today or earlier and its hard dependencies are satisfied; a later date or reopened dependency undoes it, and stored `done` or `skipped` wins. Other pinned milestones must be marked done to open gates. Until its day a pinned or `auto_done` milestone is scheduled; one with neither is ready. Marking done early is allowed and records when.
- **F2** One constraint model. Deliverables and actions have a start and a finish (starting records the start date, editable; stopping clears it). A group's come from its contents and stage bounds, with no estimate. A decision has one instant, when decided; a milestone and `created_at` have one. Every timing relation is a **constraint** "B is at least k days after A" (k any integer): date rules (one per source), dependencies (the dependent starts after the requirement finishes, implicit and inherited included), estimates (finish at least estimate after start; zero if none), containment (children finish before the parent's own work starts), stages (contents start after `opens_at`; the group finishes by `closes_at`). A pin fixes an instant. Constraints from not-relevant nodes are dropped; through undecided nodes, kept and labeled conditional. Effectively skipped work has zero duration. F3, F5, and F6 read this one network.
- **F3** Each instant has a **latest** date (a node's **due** for its finish, **latest start** for its start) and an **earliest** (**earliest start**); every constraint bounds both ends. Earliest bounds run forward from pins, actuals, and today (unstarted work starts no earlier than today; started work keeps its start and finishes no earlier than today; milestones and groups are held by their unfinished dependencies and children, not today directly); latest bounds run backward from pins and actuals. A bound no pin, actual, or today reaches is null. A pinned node's due is its pin. Each bound is the tightest allowed and carries its chain ("decision meeting Oct 10, final review closes 14 days before, report takes 3 days").
- **F4** Stages: a group may name `opens_at` and `closes_at` milestones outside it. The stage requires its opening milestone and its contents inherit the gate; `gates: false` drops the gate, leaving the opening to date only contents not yet started. The closing milestone bounds the group's finish (`closes: false` drops it). An unanchored stage has null latest bounds; its earliest bounds still come from its contents.
- **F5** Plan consistency is an invariant: no **contradictory chain**, a loop of constraints needing more time than it allows (seven days of sequential work in a five-day window; two rules placing milestones before each other). A patch creating one, a feeding date answer included, is rejected listing the chains up to a fixed number (saying if there are more), each with constraints, sources, pins, and shortfall in days. Resolution is an ordinary patch (`shift <delta>`, `repin <date>`, `unpin`, change an offset or estimate, restructure), offered inline; nothing shifts automatically.
- **F6** Reality is never rejected, and recorded dates never warn. Actuals and today layer over the consistent plan: an actual replaces its milestone's pin wherever referenced. In this layer earliest and latest bounds are computed separately and never reconciled, producing flags instead: `overdue` (unfinished, due before today) and **`shortfall`** (unstarted work whose latest start has passed, or started work that cannot finish in time, or remaining work a recorded date leaves too little time for: earliest past latest). Shortfalls come only from the plan ahead: a finished node is held by nothing of its own, so finishing early, ahead of a dependency, or late is never a shortfall on it or what it went ahead of, while other nodes' rules that name it still hold remaining work. Started work's dependencies and own date hold its finish, not its start, and a non-gating stage opening no longer holds it. Each shortfall carries its chain and F5's resolutions.
- **F7** Node detail, the list, the timeline, and the API distinguish pin, actual, and derived dates; show earliest start, latest start, due, slack, and shortfall with chains (conditional links labeled); and name the pin or rule that moves a bound. No capacity or leveling.

### G. Notes, links, and artifacts

- **G1** Any node, of any relevance or state, and the journey take markdown notes and typed links (artifact, reference, conversation): attributed, timestamped, editable, deletable, evented.
- **G2** `requires_artifact` makes `has_artifact` a guard; an artifact link satisfies only its own node. Removing it after completion makes the node `stale` ("missing artifact").
- **G3** Message drafts render with journey context and can be copied; Cairn never sends.
- **G4** `requires_note` needs a journey note on the node itself (not a link, a child's note, or a journey note). Removing the last after completion makes it `stale` ("missing note").
- **G5** A checklist item has a node-unique slug `id` and a `title`; at most 50. A patch op sets an item's tick; setting the current state is a no-op (no event). An unknown item is refused. Ticks are journey state: they survive reopening, and removing an item (edit or upgrade) removes its tick in the same write. With `requires_checklist`, `all_ticked` fails once per unticked item; an empty checklist passes; a bypass records the unticked items; an item added or unticked later makes a finished node `stale` ("unticked item"). `checklist_count` is ticked of total.

### H. Multiplayer and identity

- **H1** Pluggable authentication: OAuth/OIDC in production; a static token or named dev user locally.
- **H2** Every mutation records its actor, a user or an agent for one; applying a proposal also records the confirming user.
- **H3** An email belongs to at most one identity (a merge moves emails to the survivor). Once per request, after authentication and before anything else, Cairn finds the identities holding the caller's verified emails, creating one holding all of them (named by the login, if it gives a name) when none does. A user is the identity holding one of their verified emails; a login with none is refused. Unverified emails never match; emails compare case-insensitively after trimming. An unnamed identity shows its first email, and Cairn asks the signed-in person for a name. People are named by identity, else login name, else email; one who no longer exists is "someone no longer here", never an id. If a user's emails match two identities, "mine" covers both and they can be merged.
- **H4** No RBAC: every authenticated user reads and writes everything in the deployment.
- **H5** Optimistic locking per domain (journey, route with draft, deployment). A patch whose base revision has moved is still applied when nothing the intervening events touched is anything it writes there (only applying it can tell): it commits at the current revision and the answer names the revisions passed and what intervened. Otherwise it is rejected with both touched sets, the changes are shown, and the user retries. A proposal apply, and a patch whose mutations name revisions (the journeys a merge or delete checked), are never moved forward. Guards run at apply either way. Proposals have their own editing revisions, checked on each edit; editing one does not advance its destination. Every patch carries a client-generated patch id: resubmitting a committed id with the same content returns its receipt (the revision produced, marked already applied, no consequences; it records the patch as submitted, so this holds even when it was applied at a later revision); the same id with different content is rejected. The receipt is checked before the base revision.
- **H6** Every open view, across tabs, devices, and users (indexes, route screens, and proposals included), refreshes when its data changes, with no reload. The server announces each new revision of a journey, route, proposal, or the deployment, and a view refetches only for a revision newer than its own.

### I. AI / agent access

- **I1** Everything the UI does is in the API; the UI is its client.
- **I2** A hosted, remote MCP server (HTTP transport, an OAuth resource server sharing the pluggable auth), usable by cloud agents with no local install. Tool surface includes at least: list/get routes and journeys; create a journey (from route or empty); create/edit a route draft and publish it; get the graph at an aggregation level; get the ranked frontier, open decisions, needs breakdown, unassigned, active, blocked, stale, overdue, shortfall, and "mine"; get node detail with priority and date explanations; answer decisions; transition nodes; snooze/unsnooze; assign participations; apply a patch; draft, list, get and edit proposals; pin/unpin dates and resolve conflicts; force-include, keep under skipped ancestors, and complete past guards; list identities by name or email, find the journeys that refer to an identity or that it owns, and manage identities (create, link, merge, delete); route import/export; save as route; re-link; upgrade; segments through the route tools (filtered by kind), insert a segment, upgrade an insertion, save as segment, and link an insertion; search.
- **I3** The snapshot is the primary agent read and stays small at any size: name, description, status, owners, whether completion is suggested, and counts (by display and by stored state; on the acting frontier, decisions to answer now, placeholders to break down, unassigned, with a shortfall, what a stalled journey waits on). It lists no nodes: each count is the total of the node list under the matching filter (ranked, scoped by subtree and depth, paged), and the node read has the rest (C8).
- **I4** Agent instructions ship and version with the API (authoring routes, structuring journeys, walking decisions, proposing breakdowns, recording answers, summarizing the frontier), served through MCP instruction metadata so any connected agent gets them.
- **I5** In-app assistant: a conversational panel on the same API and instructions. Structural changes go through proposals (C14); state changes the user asks for (transitions, answers, notes, assignments, pins, snoozes) it writes directly and reports with consequences, unless asked for a proposal or the change is large or ambiguous. The model provider is deployment-configurable and vendor-neutral; the assistant is optional.
- **I6** Any agent can create a proposal against any domain, including a new journey or route at revision 0; any user can review, edit, apply, or discard it. Apply checks the destination base and the reviewed proposal revision, validates, and commits mutations, events, and applied status atomically. On a stale base the intervening changes are shown and the refreshed proposal is reviewed again. The proposing agent and confirming user are recorded. Proposal ids are client-generated, so a lost response is recovered by id under the destination.
- **I7** Every agent has its user's capabilities and is reviewed wherever its user is; applying an upgrade needs the same confirmation for anyone.

### J. Audit log

- **J1** Every mutation appends an event (a deletion's goes in the deployment log; route events carry the route's kind; a bypassed completion carries its bypass): actor (and agent), time, subject key, the after-state delta needed to replay it, optional note; one patch's events share its patch id and order. Events never store derived fields.
- **J2** One write path, the patch applier, writes state and events in one transaction; nothing writes the database outside it. A test asserts every mutation type emits an event.
- **J3** A replay harness rebuilds state from the log and compares it field by field with stored state after every accepted step of the scenario tests, which cover every feature's hard cases (answer changes after progress, cascades, contradictory chains and their resolutions, upgrades, merges, insertions, snoozes, stale proposals, concurrent patches, and the like). Drift fails the build. Scenarios also assert bounds, shortfalls, blocking, and ranking at a fixed clock, which replay alone cannot.
- **J4** History is viewable per node and per journey, grouped by patch.
- **J5** Events are queryable through the API by journey, node, user, type, patch, and time range.

## Invariants

Enforced on every patch; a violating patch is rejected with every violation and nothing is stored. Derived flags (overdue, blocked, stale, unassigned, undecided, snoozed, needs breakdown, shortfall) are not violations; guards apply only to transitions (D4).

Graph:

- Ids are sibling-unique slugs; paths resolve; every reference exists and is of the right kind. No route id is `new` (the app's addresses reserve it).
- Containment is a tree; decisions and milestones have no children.
- Dependencies, implicit gates included, are acyclic. No explicit edge joins a node to its ancestor or descendant, or duplicates a `relevant_when` reference.
- The date constraints (and a journey's pins) contain no contradictory chain (F5).
- Conditions use the supported operators on decisions of a compatible type, never a decision in the node's own subtree.
- Stage bounds reference milestones; estimates, offsets, and weights are non-negative and finite.
- Participation kinds used are declared (or `owner`); no single-valued kind on a multi-valued role; `fills_role` cardinality matches; a role has at most one filling decision and a milestone at most one feeding decision.
- Field kinds per A1a: `placeholder`, `requires_note`, `checklist`, `requires_checklist` on deliverables and actions; `requires_artifact` on deliverables; `feeds_milestone` on date decisions, naming a milestone; `auto_done`, `final` on milestones, at most one `final` per graph. A tick names an item on its node's checklist.
- A segment has no `final` or `default_owner` and at most one root (exactly one in a version); a route's kind never changes. An insertion is on an existing published segment version, has at least one member, and keeps cardinality in its maps; a node is in at most one insertion; in a journey a node is `from_segment` exactly when it is a member. No journey is created from or lineaged to a segment.
- Keys are unique within a graph and never reassigned: a graph remembers every retired key.

Journey:

- Answers match their decision's type; identity answers and owners resolve to identities.
- A journey's lineage, if any, references one existing route version.
- A patch's base revision equals its target's current revision (domain or proposal), subject to H5.
- An archived journey accepts only a move back to `active` or `completed`.
- A deleted journey's or route's id is never reused; a route is deleted only while unused (A19).

## Non-functional requirements

- **Config-first**: an author with no code access adds a node in minutes. Validation errors carry paths and are fixable without reading source.
- **Journey durability**: route edits never touch a journey until an upgrade is confirmed.
- **Structured storage**: graphs and journeys are structured, queryable data with constraints, never opaque blobs.
- **Small-team scale**: tens of users, hundreds of journeys, thousands of nodes per deployment; whole-journey derivation (dates and consistency over the whole network at once, each read) feels instant.
- **Deployable**: one service with a database, one command locally, a small cloud footprint; the assistant optional.
- **Portable data**: routes export in a documented format.
- **Observable**: structured logs and basic metrics for the API, MCP, and assistant.
- **Domain-free**: code, schema, fixtures, tests, comments, shipped docs, and agent instructions never name a specific use case Cairn serves or the people, teams, or organizations behind one, even when a conversation does. Examples are invented and generic (the vendor evaluation below, a hiring loop, a product launch). Naming technologies Cairn uses is fine.
- **Naming in code**: "route" is the process template; a framework's route is qualified, never the concept renamed.

## Illustrative example (generic)

A "vendor evaluation" route, the shape the fixtures and tests use:

- Up-front decisions: purchase or research only (choice); evaluation owner (identity, fills `eval_owner`, also `default_owner`); who to keep informed (identity list, fills `stakeholders`); decision-meeting date (pins `decision meeting`). A new journey's decision walkthrough opens on these four.
- Gated decisions: "which comparison set" requires the test plan, so the comparison baseline is undecided until then; "who reviews the findings" fills `findings_reviewer` after the findings writeup.
- Groups Setup, Testing, Reporting. Setup is a stage opened by a "kickoff" milestone. Testing holds a Partner-led subset (review the partner's criteria, read the partner's results) relevant only when "partner runs testing?" says so.
- Work: environment access; test plan (requires access; two child actions); test workload (placeholder); comparison baseline; findings writeup; final report (weight 5).
- "Decision meeting" (externally owned, weight 10) closes a stage "final review" whose `opens_at` is `due_by: {before: decision meeting, offset: 14}`. The report's due derives from the close: an earlier report pin is fine, a later one is rejected with the chain. With the meeting pinned, environment access outweighs the plan in gravity (gravity never decreases upstream between in-scope nodes); slack and unlocks separate the plan from other early work.

## Later

Deferred: sign-off; recurring items; per-member fan-out; soft `follows` edges; a CLI; authorable state machines; reminders; a markdown summary export; presence and co-editing; object-scoped concurrency; link previews; journey file import/export and restore; pinned canvas positions; RBAC; default answers (rejected for now: they hide that a decision is open); cross-journey priority; learned priority; merging user accounts; per-user assistant credentials; a shared database and several instances; patch revert; binding segment nodes to host nodes; saving an insertion's edits back to its segment; carrying insertions through save as route; bulk insertion upgrades; checklist item owners and promotion to nodes.

## Success criteria

- A participant knows within a minute of opening a journey where they are, what's next and why, what needs deciding, what's theirs, what's blocked, and what's overdue.
- An author gets a reviewable route draft by describing a process to the assistant in ten minutes, or adds a conditional deliverable or a gated decision without code.
- Someone with no route gets from an empty journey and a description to an applied structure and a ranked frontier in ten minutes.
- An agent over remote MCP starts a journey, answers its decisions, proposes a breakdown, and sees the frontier the UI shows.
