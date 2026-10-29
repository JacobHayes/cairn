# The assistant is a loop over the tools and one chat-completions client

Question (from the assistant cut): the in-app assistant had three provider protocols, a policy layer that classified each write and redrafted structural ones as proposals, a dry run of every write to find the nodes it touched, a per-turn snapshot, a state machine of ten endings, and a report written for each action. Against the same guide and tools an external agent uses, it adds one thing: the model's calls run in process. How much of it earns its keep?

Call: none of the layers. The assistant is a loop:

- **One provider protocol, written by hand.** One OpenAI-compatible chat-completions endpoint (OpenRouter, a gateway, a local server), one non-streamed JSON POST per model call over the HTTP client already in the tree. Prompt caching is the gateway's. A model with a native-only API is reached through such a gateway.
- **No retries.** A failed call ends the turn, stopped with the reason; the writes before it stand. The person sends the message again.
- **The model is offered the MCP tool list**, less the tools whose writes are the user's to do (a proposal's confirmation, a file import); a call to any other name is answered with a refusal, which counts toward the repeated-failure stop. The system text is the shipped guide with a short preamble naming the user and the target.
- **No policy layer.** The guide says when a change is direct and when it is proposed, as it does for an external agent.
- **Reports come from the tool's answer.** A write is reported from the answer the tool gave (the patch, where it landed, the revision, what it caused; a proposal's id and destination and whether it was discarded) with the patch's target from the call's arguments. The conversation keeps one plain line for each, in words and naming no id (the screens never show one), so a reload shows the same words; what a write caused is the reply's to say, as the guide tells an agent. The panel shows the stored conversation and, for the last turn, links to the proposals it drafted.
- **No turn snapshot.** The model reads the target through tools when it needs to.
- **Limits.** One turn at a time per conversation, turns in flight, provider call, turn duration, iterations and tool calls per reply, tool-result bytes, the replayed dialogue, and the save reserve. A reply that did not finish (cut at the output limit or filtered) runs none of its calls.

Alternatives: a provider library (`genai`, `rig`, `async-openai`): each brings its own tool and message types over a protocol this uses a fraction of, and the types must still be converted; Anthropic Messages and OpenAI Responses protocols (two more wire formats to hold against provider changes, for gateways that already translate); a policy layer (a second classifier beside the engine's and the guide).

What would change it: a model that reads the guide and proposes badly often enough to cost users (then a stricter guide or a check on the result, not a layer in front), or a deployment that needs a native API a gateway does not offer.
