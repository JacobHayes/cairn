# The binary has a demo mode, with a stub sign-in and a scripted assistant

Question (from the browser tests' hosts): the server-host tests ran against a second server, `fixture_server`, that re-assembled the composition root over a memory store, beside the real binary, with its own Vite dev server proxying to it. Can one binary and one Vite server do all of it?

Call: `cairn demo [--listen ADDRESS]` serves the sample journeys, the fixtures seeded into a memory store at start (so nothing is saved), signed in by the dev provider, on loopback, with no configuration. Only in this mode, a stub OIDC issuer and a scripted assistant answer under `/demo/` on the same port, and the demo configures the server to use them as any deployment would (an `oidc` provider at its own address, a `chat_completions` assistant at its own address). So the server-host tests (live updates, conflicts, sign-in, assistant) run against the binary's own composition root, API, Host allowlist, and embedded web build; `fixture_server`, `seed_fixtures`, `scripts/built`, `scripts/e2e-binary`, and the second Vite server are gone, and `README` "Try it" is one command. The demo's cost is about 330 lines in the shipped binary and the fixtures (80 KB), `ed25519-dalek`, and `cairn-wasm` as normal dependencies of `cairn`.

Alternatives: a test-side sidecar for the issuer and the model beside a binary over a seeded database file (the audit's recommendation: nothing test-shaped ships, but a second program and a seeding step stay); keeping the fixture server (two compositions to keep equal).

What would change it: a deployment that must not carry the stub (put `demo` behind a cargo feature then), or a stub-issuer bug that hides a real OIDC one (the issuer's own tests in `crates/tests` keep the adversarial cases).
