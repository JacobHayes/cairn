# Cairn: feature briefs

Briefs are Cairn's implementation milestones: what each builds, in what order, and how it is shown to work, with its decision log and its proof under `briefs/proof/<id>/`. Phases 1 to 6 have landed and are the record of the milestones that built Cairn; read the ones that built what your work touches. Phases 7 and 8 are planned: each of their briefs is a unit of work for whoever picks it up, built and proved the way the landed ones were, and marked landed in the table below in the same commit. Work that no brief covers starts from the documents alone (`AGENTS.md`), and its proof sits beside the briefs' proofs, under `briefs/proof/YYYY-MM-DD-<short-slug>/`. A brief points into `PRD.md`, `ARCHITECTURE.md`, and `PRACTICES.md` by requirement id and section heading and does not restate them; where a brief and a document disagree, the document wins (`AGENTS.md`).

Files are named `<phase>.<step>-<slug>.md`. Phases run in order; within a phase, steps run in order unless the table says otherwise. 4.8 and 4.9 finish what 4.1 and 4.2 start once the engine is complete, so they run before 4.3 and 4.5, which depend on them ([`decisions/2026-10-06-the-service-and-api-split-at-the-engines-2-4-boundary.md`](../decisions/2026-10-06-the-service-and-api-split-at-the-engines-2-4-boundary.md)).

## Template

The shape every brief follows.

1. **Header**: id, title, crates or packages, depends on.
2. **Goal**: one paragraph.
3. **Governing**: PRD ids; ARCHITECTURE and PRACTICES sections by heading.
4. **Scope**: in, and out where a neighbor could build the same thing.
5. **Acceptance**: behavior by PRD id and scenario; the planted bug; the rungs this brief adds to the ladder, if any (PRACTICES, Growing the ladder); fixtures used. Every brief also leaves `briefs/proof/<id>/` (AGENTS, Leave proof).
6. **Decisions left to the implementer**.
7. **Decision log**: appended by the implementer, for small calls; calls that matter go in `decisions/`, one new file per decision (AGENTS).

## The cut

| Id | Title | Crates or packages | Depends on | Status |
|---|---|---|---|---|
| 1.1 | Workspace, lints, tasks, CI | root, `mise.toml`, `.github/` | - | landed |
| 1.2 | Schema and fixtures | `crates/schema`, `fixtures/`, `schema/` | 1.1 | landed |
| 1.3 | Simulation spike: tokio and axum under the shim | `testbeds/spike` | 1.1 | landed |
| 2.1 | Engine core: model, apply, state machines, events, replay | `crates/engine` | 1.2 | landed |
| 2.2 | Relevance, effective dependencies, effective skip, participation | `crates/engine` | 2.1 | landed |
| 2.3 | Date network | `crates/engine` | 2.2 | landed |
| 2.4 | Auto-reach, blocking, frontier, snooze, stalled, stale, derived guards | `crates/engine` | 2.3 | landed |
| 2.5 | Gravity, leverage, rank | `crates/engine` | 2.4 | landed |
| 2.6 | Projections | `crates/engine` | 2.5 | landed |
| 2.7 | Route files, upgrade, save-as-route, re-link, proposal documents | `crates/engine` | 2.6 | landed |
| 3.1 | Store trait, memory and Turso backends, conformance suite | `crates/store`, `crates/store-turso` | 1.2 (beside phase 2) | landed |
| 3.2 | Auth providers, users, identities, agent tokens | `crates/auth` | 3.1 | landed |
| 4.1 | Service layer, composition root, capabilities, notifier | `crates/service`, `crates/store` (notifier) | 2.4, 3.1 | landed |
| 4.2 | HTTP API, OpenAPI, SSE, TypeScript client | `crates/api`, `openapi/`, `web/client` | 4.1, 3.2 | landed |
| 4.3 | MCP server and shipped instructions | `crates/mcp`, `instructions/` | 4.9 | landed |
| 4.4 | Assistant | `crates/assistant` | 4.3 | landed |
| 4.5 | Wasm host: engine package, derive worker, in-browser root | `crates/wasm`, `web/wasm` | 4.9 | landed |
| 4.6 | Web client and app shell | `web/client`, `web/app` | 4.5 | landed |
| 4.7 | The binary | `crates/cairn` | 4.4, 4.6 | landed |
| 4.8 | Service completion: priority, projections, proposals, route files | `crates/service` | 2.7, 4.1 | landed |
| 4.9 | HTTP API completion: projections, proposals, route files | `crates/api`, `openapi/`, `web/client` | 4.8, 4.2 | landed |
| 5.1 | Node detail and explanations; notes, links, artifacts | `web/app` | 4.6 | landed |
| 5.2 | Canvas, semantic zoom, trace, layout | `web/app` | 5.1 | landed |
| 5.3 | List, next, triage, decision walkthrough | `web/app` | 5.2 | landed |
| 5.4 | Decision view, timeline, status summary | `web/app` | 5.2 (beside 5.3) | landed |
| 5.5 | Journey index and overview, route screens, entities, identity | `web/app` | 5.3 | landed |
| 5.6 | Authoring | `web/app` | 5.5 | landed |
| 5.7 | Proposal review and its flows | `web/app` | 5.6 | landed |
| 5.8 | Assistant panel | `web/app` | 5.7, 4.4 | landed |
| 6.1 | Multiplayer testbed | `testbeds/multiplayer` | 4.2, 1.3 | landed |
| 6.2 | Durability testbed | `testbeds/durability` | 4.1 | landed |
| 7.1 | Unlocked nodes first in a pass | `web/app` | 8.2, 8.9 | planned |
| 7.2 | A rationale on answers | `crates/schema`, `crates/engine`, `crates/store`, `crates/store-turso`, `crates/api`, `crates/mcp`, `openapi/`, `web/client`, `web/app` | 5.4 (screens: 8.7, 8.9) | planned |
| 7.3 | The `requires_note` guard | `crates/schema`, `schema/`, `crates/engine`, `crates/store`, `crates/store-turso`, `openapi/`, `web/client`, `web/app` | 5.6 (screens: 8.7, 8.9) | planned |
| 7.4 | Snoozing a container | `crates/schema`, `crates/engine`, `crates/service`, `crates/api`, `crates/mcp`, `openapi/`, `web/client`, `web/app` | 5.3 (screens: 8.7, 8.9) | planned |
| 7.5 | Notices: work the final milestone cannot see | `crates/engine`, `crates/service`, `crates/api`, `crates/mcp`, `openapi/`, `web/client`, `web/app` | 5.6 (draft card: 8.11) | planned |
| 7.6 | Segments: model and insert | `crates/schema`, `schema/`, `crates/engine`, `crates/store`, `crates/store-turso`, `crates/service`, `crates/api`, `crates/mcp`, `instructions/`, `openapi/`, `web/client`, `fixtures/` | 4.3 | planned |
| 7.7 | Segment screens | `web/app` | 7.6, 8.6, 8.11 | planned |
| 7.8 | Insertion upgrade | `crates/schema`, `crates/engine`, `crates/service`, `crates/api`, `crates/mcp`, `instructions/`, `openapi/`, `web/client`, `web/app`, `fixtures/` | 7.6 (screens: 7.7) | planned |
| 7.9 | Save as segment and link | `crates/schema`, `crates/engine`, `crates/service`, `crates/api`, `crates/mcp`, `instructions/`, `openapi/`, `web/client`, `web/app` | 7.6 (screens: 7.7) | planned |
| 8.1 | Display state | `crates/schema`, `crates/engine`, `crates/mcp`, `instructions/`, `fixtures/`, `openapi/`, `web/client`, `web/app` | 2.6 | planned |
| 8.2 | Priority explanations and consequences | `crates/schema`, `crates/engine`, `crates/service`, `crates/api`, `crates/mcp`, `openapi/`, `web/client`, `fixtures/` | 8.1 | planned |
| 8.3 | Level collapse and answer effects | `crates/schema`, `crates/engine`, `crates/wasm`, `crates/api`, `crates/mcp`, `openapi/`, `web/client`, `web/wasm`, `web/app` | 8.2 | planned |
| 8.4 | Frame and Graticule migration | `web/app`, `design/`, `mise-tasks/` | 5.8 | planned |
| 8.5 | Sync chip | `web/app` | 8.4 | planned |
| 8.6 | Journey pages, navigation, and addresses | `web/app` | 8.4, 8.1 | planned |
| 8.7 | Inspector | `web/app` | 8.4, 8.1 (second part: 8.2, 8.3) | planned |
| 8.8 | Graph | `web/app` | 8.4, 8.1, 8.3 (peak chip: 8.2) | planned |
| 8.9 | Next page | `web/app` | 8.5, 8.6, 8.7 | planned |
| 8.10 | Plan list and timeline | `web/app` | 8.6, 8.1 (Affects: 8.3) | planned |
| 8.11 | Authoring and proposal review in the frame | `web/app` | 8.6, 8.7, 8.8 | planned |

Rule for links between screens: a screen only links to screens that already exist. The brief that builds a screen adds the entry points into it from earlier screens (5.7 adds "break down" to triage and the upgrade, save-as-route, and re-link entries to the overview; 5.8 mounts its panel on earlier screens).

## Order

```mermaid
flowchart LR
    s11[1.1] --> s12[1.2] --> s21[2.1] --> s22[2.2] --> s23[2.3] --> s24[2.4] --> s25[2.5] --> s26[2.6] --> s27[2.7]
    s11 -.-> s13[1.3]
    s12 --> s31[3.1] --> s32[3.2]
    s24 & s31 --> s41[4.1] --> s42[4.2]
    s32 --> s42
    s27 & s41 --> s48[4.8]
    s48 & s42 --> s49[4.9]
    s49 --> s43[4.3] --> s44[4.4]
    s49 --> s45[4.5] --> s46[4.6]
    s44 & s46 --> s47[4.7]
    s46 --> s51[5.1] --> s52[5.2] --> s53[5.3] --> s55[5.5] --> s56[5.6] --> s57[5.7] --> s58[5.8]
    s52 --> s54[5.4]
    s44 --> s58
    s42 & s13 -.-> s61[6.1]
    s41 -.-> s62[6.2]
    s26 --> s81[8.1] --> s82[8.2] --> s83[8.3]
    s58 --> s84[8.4]
    s84 --> s85[8.5]
    s84 & s81 --> s86[8.6]
    s84 & s81 --> s87[8.7]
    s84 & s81 & s83 --> s88[8.8]
    s85 & s86 & s87 --> s89[8.9]
    s86 & s81 --> s810[8.10]
    s86 & s87 & s88 --> s811[8.11]
    s82 & s89 --> s71[7.1]
    s54 --> s72[7.2]
    s56 --> s73[7.3]
    s53 --> s74[7.4]
    s56 --> s75[7.5]
    s43 --> s76[7.6]
    s76 & s86 & s811 --> s77[7.7]
    s76 --> s78[7.8]
    s76 --> s79[7.9]
```

Phase 8 redesigns the web app and adds what it needs from the engine. 8.1 to 8.3 (engine) and 8.4 (the frame) start side by side; 8.1, 8.2, and 8.3 all touch the derived schema and the generated paths, so they land in that order and regenerate on rebase. 8.5 to 8.8 follow the frame, and 8.9 to 8.11 follow them. Phases 7 and 8 are the exception to phases running in order: phase 7's briefs are independent of each other apart from 7.7 to 7.9 on 7.6 (and the screens of 7.8 and 7.9 on 7.7), 7.1 waits for 8.2 and 8.9, and the model, API, and MCP parts of 7.2 to 7.5 can run beside phase 8 in separate jj workspaces while their screens land with or after the phase 8 briefs the table names. Dotted: the simulation track (1.3, 6.1, 6.2). It is exploratory and gates nothing (PRACTICES, Simulation): no other brief depends on it, it runs under `mise run sim`, never `mise run check`, and it can be picked up whenever its dependencies exist or skipped if the spike says the shim cannot carry it.

## Coverage

Every PRD requirement id and named section, mapped to the brief that owns its acceptance. Other briefs may touch an id and cite it under Governing.

| PRD | Owner | Also touched by |
|---|---|---|
| Identity and references | 1.2 | 2.1 (import matching, key minting) |
| Containment | 2.2 (effective graph), 2.4 (what it blocks) | 2.1, 2.3, 2.5 |
| Gating | 2.2 (relevance), 2.4 (blocked, frontier) | - |
| Priority | 2.5 | 2.6, 5.2, 5.3, 8.2 (peak gravity, still waiting) |
| Invariants: graph structural | 2.1 | 2.2 (cycle check over the effective graph), 2.4 (snooze wait cycles) |
| Invariants: date network | 2.3 | 2.1 |
| Invariants: journey state | 2.1 | 3.1 (revision), 4.1 |
| A1, A1a, A14 | 1.2 | 2.1, 3.1 |
| A2, A3, A4, A6, A7 | 2.1 | 2.2 |
| A5 | 2.2 | 1.2, 2.1 |
| A8, A9 | 2.3 | 1.2, 4.7 |
| A10 | 2.6 | 1.2, 5.1 |
| A11 | 4.1 | 2.1, 5.5 |
| A12 | 5.6 | 4.1, 4.3, 4.4 |
| A13 | 2.7 | 2.1 (fixtures load), 4.8, 4.9, 5.5, 7.6 (segment files) |
| A15, A16, A17, A18 | 2.1 | 1.2, 4.1, 4.2, 7.3 (`requires_note`), 7.5 (notices), 7.6 (a segment is a route) |
| A19 | 4.1 | 3.1, 5.5, 7.6 (retiring a segment) |
| A20 | 7.5 | 8.11 (the draft card) |
| A21 | 7.6 | 7.7 |
| B1, B2, B3, B5, B10, B11 | 2.1 | 2.2, 4.1, 5.3, 5.5, 5.6, 7.2 (rationale) |
| B4 | 2.1 | 5.6, 2.7 |
| B6 | 2.4 (actionable nodes), 7.4 (containers) | 2.1, 5.3 |
| B7, B8, B9 | 2.7 | 4.8, 4.9, 5.7, 7.8 (the merge, reused), 7.9 (save as route drops insertions) |
| B12 | none (reserved) | - |
| B13 | 7.6 | 7.7 |
| B14 | 7.8 | 7.7 (upgrade available) |
| B15 | 7.9 | - |
| C1, C3, C4, C5, C6, C7, C15 | 5.2 | 2.6, 8.8 (the redesigned graph) |
| C2 | 2.6 (roll-up rules) | 5.2, 8.3 (collapse and display states in the level), 8.8 (the ladder) |
| C8 | 5.1 | 2.6, 7.2, 8.1 (pending cause), 8.2 (still waiting), 8.3 (answer effects), 8.7 (the inspector) |
| C9, C10, C11 | 5.3 | 2.6, 7.1 (pass order after an action), 7.4, 8.6 (pages and filters), 8.9 (Next page), 8.10 (Plan list) |
| C12, C13, C18 | 5.4 | 2.6, 7.2 (C12), 8.6 (decisions filter, Summary page), 8.10 (decision list, timeline) |
| C14 | 5.7 | 2.7, 4.8, 4.9, 8.11 |
| C16, C17 | 5.5 | 3.1, 2.6, 8.6 (journey card, index), 8.9 (Mine), 8.11 (route detail) |
| C19 | 7.7 | 7.6 (route detail read), 7.8 (upgrade entries), 7.9 (save from a selection) |
| D1, D1a | 2.1 | 2.2 |
| D2, D4, D5, D7 | 2.4 | 2.1, 4.1, 4.6, 7.3 (`has_note`), 7.4 (D5), 8.2 (D7's informational half) |
| D3, D6 | 2.2 (the `Derived` struct) | every engine brief; 4.8 (memoized reads); 8.1 (`display_state`) |
| D8 | 8.1 | 7.4 (`snoozed_via`), 8.6 to 8.10 (every surface reads it) |
| E1, E2, E5 | 2.2 | 8.1 (E1 narrowed) |
| E3 | 2.1 | 5.1 |
| E4 | 2.6 | 2.2 |
| E6 | 2.1 | 3.1, 4.1, 5.5 |
| F1 to F7 | 2.3 | 2.2, 2.4, 5.1, 5.4 |
| G1, G2 | 2.1 | 5.1, 5.5 |
| G3 | 2.6 | 5.1 |
| G4 | 7.3 | - |
| H1, H4 | 3.2 | 4.1 |
| H3 | 4.1 | 3.2 (verified emails), 2.2, 5.5 |
| H2 | 4.1 (the actor), 4.8 (the confirming user) | 3.2, 4.4 |
| H5 | 3.1 | 1.2, 2.1, 4.1, 4.2, 6.1 |
| H6 | 4.6 | 4.1, 4.2, 4.5, 5.5, 6.1, 8.5 (the sync chip) |
| I1 | 4.2 | 4.9 |
| I2, I4, I7 | 4.3 | 3.2, 4.8 (I7), 7.6 (segment kind and instructions), 7.8, 7.9 (segment tools), 8.1 (instructions), 8.2 and 8.3 (node detail fields) |
| I3 | 2.6 | 4.8, 4.9, 4.3, 8.1 (`by_display_state`) |
| I5 | 4.4 | 5.8 |
| I6 | 4.8 | 2.7, 4.9, 5.7 |
| J1, J2, J3 | 2.1 | 3.1, 6.1, 6.2, 7.2, 7.4, 7.6, 7.8, 7.9 |
| J4 | 2.6 | 5.1 |
| J5 | 3.1 | 4.2 |
| Non-functional: Config-first | 5.6 | 4.4 |
| Non-functional: Journey durability | 4.1 (publishing never touches journeys), 4.8 (an upgrade only once confirmed) | 2.7 |
| Non-functional: Structured storage | 3.1 | - |
| Non-functional: Small-team scale | 2.3 (cost test at the limits) | every engine brief |
| Non-functional: Deployable, Observable | 4.7 | 4.2 |
| Non-functional: Portable data | 2.7 (route file round trip, save as route) | 4.8 |
| Non-functional: Domain-free, Naming in code | review of every brief | - |
| Illustrative example | 1.2 (fixtures) | every engine brief |
