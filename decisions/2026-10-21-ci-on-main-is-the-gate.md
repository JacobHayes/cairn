# CI on main is the gate

Question (from timing a unit end to end): before every push the full `mise run check` ran once on this machine, about 6 minutes, under the machine-wide check lock that every unit's fast checks also wait on, and CI then ran the same check again. It existed so main was never red. Since `2026-10-21-every-deploy-is-a-tagged-release.md`, nothing is deployed or released from a commit whose CI run has not passed, so a red main no longer reaches anyone. Does a push still need the local full check?

Call: no. A unit lands once `mise run check:fast` passes on its rebased result, and main is pushed as it lands. CI's run on main is the gate: a release is tagged only on a commit whose run passed. When the newest finished run on main failed, `mise run land` refuses to land until main is green again, except the fix itself (`mise run land --fix`), so nobody stacks work on a broken main. The owner agreed main may be red for about one CI run.

Alternatives: the local full check before every push (about 6 minutes per push, serialized on this machine, for a check CI repeats); GitHub's merge queue (main always green and nothing waits here, but every unit becomes a pull request behind branch protection, ceremony that buys little for one owner whose agents land).

What would change it: main red often enough that refused landings cost more than the local check did, or a second deployment that runs main untagged.
