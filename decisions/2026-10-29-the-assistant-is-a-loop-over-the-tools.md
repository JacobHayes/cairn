# The assistant is a loop over the tools and one chat-completions client

Question: against the same guide and tools an external agent uses, the in-app assistant adds one thing: the model's calls run in process. What else does it need?

Call: nothing beyond a loop.

- One OpenAI-compatible chat-completions endpoint, written by hand: one non-streamed JSON POST per model call. A model with only a native API is reached through a gateway, which also owns prompt caching.
- No retries: a failed call ends the turn with the reason, and the writes before it stand.
- The model is offered the MCP tool list less the writes that are the user's own (confirming a proposal, importing a file); any other name is refused. The system text is the shipped guide with a short preamble.
- No policy layer and no cap on how many nodes a write touches: the guide says when to write directly and when to propose, as it does for an external agent, and a state change the person asks for is applied and reported with what it caused.
- A write is reported from the tool's answer, as one plain line in words naming no id.
- No turn snapshot: the model reads through tools when it needs to.
- Limits bound every turn: one at a time per conversation, turns in flight, provider call and turn duration, iterations and tool calls per reply, tool-result bytes, the replayed dialogue, and the save reserve. A reply that did not finish runs none of its calls.

Alternatives: a provider library (its own message and tool types over a protocol used a fraction of); more wire protocols (formats to hold against provider changes, for gateways that already translate); a policy layer or node cap (a second classifier beside the engine and the guide, protecting only the assistant's users while external agents write uncapped).

What would change it: a model that proposes badly often enough to cost users (a stricter guide or a confirmation, not a layer), or a deployment that needs a native API no gateway offers.
