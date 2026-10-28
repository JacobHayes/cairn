# Workflows are read on request, through a tool and as prompts

Question (from trimming what an agent loads): the shipped workflows reach an MCP agent only as prompts, which many clients offer to people and not to the model, and the in-app assistant carried every workflow's full text in its system prompt on every request. How should an agent read a workflow?

Call: a read tool, `get_workflow`, answers a workflow's text by name, the same text its prompt serves. Its `name` is an enum of the workflow names, and its description lists each one with what it is for, both taken from the workflows' own front matter, so an agent finds them from the tool list alone. The guide's opening says to read the workflow that fits before doing its job. The prompts stay, so the guide still reaches every agent through the protocol's own metadata. The assistant's system prompt lists each workflow's name and purpose instead of its text, so the text enters an exchange only when the job needs it; the list stays the same on every request, so a provider can still cache it. All nine shipped workflows are served.

Alternatives: keep the prompts only (unreadable to agents whose client keeps prompts for people); fold the workflows into the guide (every agent loads every workflow on connecting); one tool per workflow (nine names for one kind of read).

What would change it: clients that hand prompts to the model, which would make the tool redundant for external agents; a workflow so short that naming it costs about what its text does.
