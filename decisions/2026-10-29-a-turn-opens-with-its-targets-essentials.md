# A turn opens with its target's essentials, not a full read

Superseded in part by `2026-10-29-the-snapshot-is-facts-and-counts-only.md`: the journey essentials.

Question (from the assistant's per-turn context): each turn led its user message with the journey's whole snapshot, or the route's whole draft. On a large journey that is up to about 25k tokens on every turn, most of it unused by a given message. How much of the target should a turn carry?

Call: only the essentials. For a journey: its key, name, status, revision, whether completing it is suggested, and the snapshot's counts. For a route draft: the route's key, name, revision, how many versions it has, the version the draft extends, and the draft's node count. The context says the rest is read with `get_snapshot`, `list_nodes`, `get_node` or `get_route` when the message needs it. Measured on compact JSON with the o200k_base tokenizer, the context of a journey went from 380 to 560 tokens on the small fixtures to about 180, and from 10.5k tokens on a generated journey of 400 independent items to about 160. The cost of a lookup is one tool call, only on the turns that need it.

The web client sends only the message, so nothing on screen (the open node) is in the context; the request carries no such field and none was added.

The system text also dropped its own list of workflows, which the guide it embeds already gives: about 1,200 bytes (roughly 250 tokens) less on every request.

Alternatives: cap the frontier list in the snapshot (still grows with the journey's other lists, and still sends detail most turns ignore); send everything (the cost above, every turn). The earlier call, that a turn's context is not stored with its message, stands and is cheaper still: what is not stored is now small.

What would change it: turns that nearly always begin with the same lookup (then carry that part), or the web client sending what the person has open (then name it in the context).
