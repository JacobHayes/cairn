# Rung 1 applies formatting and machine-applicable fixes locally; CI only verifies

Question (from the validation ladder's speedups): a formatting slip or a lint with a mechanical fix failed rung 1, and the author, usually an agent, spent a run to learn what the tools could have done themselves. Should the check repair what a tool can repair?

Call: outside CI (`CI` unset) rung 1 first runs `cargo fmt --all`, `cargo clippy --fix` over the same targets, features, and raised lints it then checks (machine-applicable suggestions only), `cargo fmt` again, and `eslint --fix` (with its cache), and prints `rung 1 fixed: N files`; then every check runs unchanged, so only what needs a person's decision fails. With `CI` set nothing is rewritten and the rung verifies as before, so a change pushed unformatted still fails there. The CSS lint has no fixer, and tsc has none.

Alternatives: a separate `fix` task (one more thing to remember, and the rung would still fail first), fixing in CI too (the branch under test is not the one the author reads; a check should not change what it checks), and parsing the fix run's output as the clippy check (saves about four seconds, but changes what the check trusts).

What would change it: fixes landing in files the author did not touch (the count line is the signal), a fixer that changes behaviour (clippy keeps machine-applicable only), or a local run slow enough that the fix pass (about six seconds more, mostly `clippy --fix`, which does not reuse the check's build) is not worth it.
