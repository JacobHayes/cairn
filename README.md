# Cairn

Cairn structures a process as one graph of nodes that must be decided or done: decisions,
deliverables, actions, milestones, and groups. A route is the reusable template; a journey is
a live graph with answers, progress, people, and dates. Every change is a validated, atomic
patch, and Cairn ranks the frontier to answer "what should I do next?". People use it through
a web UI; agents use the same capabilities through an HTTP API and an MCP server.

Cairn is being built in feature briefs; nothing runs yet beyond the checks.

## Documents

- [`PRD.md`](PRD.md): what Cairn does, and its glossary.
- [`ARCHITECTURE.md`](ARCHITECTURE.md): how it is built.
- [`PRACTICES.md`](PRACTICES.md): how code is written and verified.
- [`AGENTS.md`](AGENTS.md): the entry point for implementers, then [`briefs/`](briefs/README.md).
- [`DECISIONS.md`](DECISIONS.md): judgment calls awaiting review.

## Develop

Tools are pinned in `mise.toml`; tasks live in `mise-tasks/`.

```sh
mise install          # pinned Rust toolchain, Node, and wasm tool
mise run check        # every rung of the validation ladder that exists
mise run check:fast   # rungs 1 to 3: the inner loop
mise run check:1      # one rung
mise run gen          # regenerate the generated paths
```

CI (`.github/workflows/check.yml`) runs `mise run check` on every push and nightly.

## License

[Apache-2.0](LICENSE).
