# The assistant reports what it used, stops what runs away, and writes directly

Someone running Cairn can now see what each model call cost, and a person talking to the
assistant is told plainly when a reply cannot be trusted or the assistant is not set up.

## What changed, observed on the fixtures

| Situation | Before | Now |
|---|---|---|
| A provider call used 10 input, 20 output, 300 cache-read and 4000 cache-write tokens | nothing recorded | one counter per kind, and a log line with the tool names the call used |
| Eleven weights changed in one request | drafted as a proposal, nothing landed | applied as one patch, reported with all eleven nodes |
| The same patch asked again | reported as a change made now | reported as already applied |
| The provider answers 429, 500 or 529 | the turn ended | retried twice with jittered backoff, honouring `Retry-After` up to 30 s |
| The provider answers 401 | "the model provider failed (message)" | "the assistant is not set up", with the provider's text kept out of the replayed conversation |
| A reply cut off at its output cap, a refusal, a paused reply | treated as a normal reply, its tool calls run | the turn ends with its own status; none of its tool calls run |
| The same failing tool call three times | ran until the 32-iteration limit | the turn ends at the third failure |
| Four rounds of 32 snapshot reads | every result resent on every call | past 256 KiB the oldest results are replaced by a note |
| A second message while a reply is in progress | "assistant is busy" | a conflict that says a reply is still in progress |
| An Anthropic or OpenAI assistant configured without a key | failed when someone sent a message | refused when the configuration is read |

The turn counter carries a status class (`none`, `4xx`, `5xx`) beside how the turn ended. An
OpenAI model that does not reason refuses being asked for encrypted reasoning; the provider
learns that from the first refusal and stops asking.

## Known limits

- A retry waits inside the provider call's 120 s limit, so a slow first attempt leaves less time for retries.
- Earlier turns' context is not stored with their messages, so the previous message is written to the provider's cache again each turn; the reasons are in `decisions/`.
