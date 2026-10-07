# Proof for brief 4.4: Assistant

A deployment that configures a model provider now offers an in-app assistant for journeys
and route drafts, and says so in its capabilities; one without a provider serves neither.
Every write the assistant makes passes one wrapper: structural changes, and state changes
touching more than ten nodes, become one proposal for the user to review.

## A conversation

Run with a scripted model standing in for the provider:

| Ann asks | The assistant | Revision after |
|---|---|---|
| Set up a two-week bake-off (an empty journey with no route) | drafts the structure as one proposal | bake-off stays at 1 until Ann applies it, then 2 |
| Answer the vendor evaluation's up-front decisions, meeting on the 6th | applies them directly and reports what they caused: nine nodes now overdue, `n_access` among the shortfalls | evaluation 1 to 2 |
| Lower the weight of every work item to 2 | eleven nodes, so one proposal; nothing lands | stays at 2 |
| Snooze the access request, then say what is next | snoozes it; the provider then stops answering and the turn ends as timed out after the 120 s provider call limit | 3, the snooze stands |

- The applied bake-off proposal's events name the assistant acting for Ann, and Ann as the
  user who confirmed it.
- Without a provider, capabilities say `assistant: false` and both assistant endpoints
  answer 404; with one, they say `true` and both answer a turn.

## Known limits

- The three wire protocols (Anthropic Messages, OpenAI Responses, chat completions) are
  checked against fixtures written from each provider's public API reference, not captured
  live, since a live capture needs a credential.
