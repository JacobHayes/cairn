# The check ladder runs the testbeds' unit tests

Question (from the nightly `sim` job going red on 2026-10-20): merging several identities at once changed a merge's `merged` from one key to a list. The multiplayer testbed writes its patches as YAML, so it still compiled, and the ladder's lint step, which formats and lints each testbed (decisions/2026-10-18-the-ladder-lints-the-testbeds.md), passed. Only the testbed's own unit test parses those patches, and only `mise run sim` ran it, so main stayed red for a night and every change landed after it inherited the break. How does the ladder catch a break a testbed shows only when it runs?

Call: the unit step runs `cargo test --locked` in each testbed after the workspace's tests, one suite per testbed, so a testbed whose tests fail or vanish fails the step. `check:fast` runs it whenever a Rust input changed, as it runs the rest of the step. CI's rust part caches the testbeds' target directories as the lint part does. `sim.sh` leg 1 still runs the same tests, so `mise run sim` stays a whole run on its own. Measured warm on an Apple M4, both testbeds' tests add about 3 s; a cold cache adds their build (about a minute each).

Alternatives:
- Build the testbed's patches from the schema's types instead of YAML text: the compiler would catch this break, but the testbed writes YAML on purpose, as a person or agent would, so each patch is one the API documents; and a break that is not a type change would still wait for the nightly.
- Keep relying on the nightly: main stays red for up to a day.

What would change it: the testbeds' tests growing slow enough to matter in the inner loop.
