# Cairn: planned briefs

A brief is a planned unit of work: what it builds, its acceptance, and a decision log the implementer appends to. A brief points into the documents by requirement id and section heading and does not restate them; where they disagree, the document wins. Build what the brief scopes, then delete the brief in the same commit that lands it (git keeps it). Work no brief covers starts from the documents alone.

| Brief | Title |
|---|---|
| [7.9](7.9-save-as-segment.md) | Save as segment |

## Template

1. **Header**: id, title, crates or packages, depends on.
2. **Goal**: one paragraph.
3. **Governing**: PRD ids; ARCHITECTURE and PRACTICES sections by heading.
4. **Scope**: in, and out where a neighbour could build the same thing.
5. **Acceptance**: behaviour by PRD id and scenario; a real bug its tests must catch; fixtures used.
6. **Decisions left to the implementer**.
7. **Decision log**: small calls; calls that matter go in `decisions/`.
