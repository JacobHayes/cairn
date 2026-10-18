# The check ladder formats and lints the testbeds

Question (from the nightly `sim` job going red): a service API change (a new `WriteError::ProposalMissing`) left both testbeds uncompilable, and nothing in `mise run check` builds them, so the change landed green and only the nightly noticed. How does the ladder keep the testbeds compiling?

Call: the lint step runs `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings` in each testbed, in place of the `cargo metadata --locked` lock check (`--locked` keeps that check). Fix mode formats and applies clippy's fixes there too, so `fix` and `check` agree. `sim.sh` leg 1 no longer repeats rustfmt and clippy and keeps the unit tests. CI's lint part caches the testbeds' target directories (rust-cache `workspaces`), so it does not compile Turso from cold. Plain clippy needs the patina SDK from git, not cargo-patina, so CI's disabled patina tool does not matter. Measured warm, the lint step went from 5.7 s to 5.6 s (the first run after a change costs the testbeds' build, about 12 s).

Alternatives:
- Keep the lock check and rely on the nightly: main stays red for up to a day and the break is inherited by whoever lands next.
- `cargo check` only: weaker than the clippy and rustfmt that `sim.sh` leg 1 already demanded, and leaves two places running clippy.

What would change it: the lint step's cold time in CI growing past what the cache saves, or a testbed needing the patina CLI to compile.
