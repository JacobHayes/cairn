# Workflows are read on request, through a tool and as prompts

Question: many clients offer MCP prompts to people and not to the model, and carrying every workflow in the assistant's system prompt costs every request. How does an agent read a workflow?

Call: `get_workflow` answers a workflow's text by name, the same text its prompt serves. Its `name` is an enum and its description lists each workflow with its purpose, both from the workflows' own front matter, so an agent finds them from the tool list alone. The guide says to read the workflow that fits before doing its job. The prompts stay. The assistant's system prompt lists each workflow's name and purpose, not its text, and stays the same on every request so a provider can cache it.

Alternatives: prompts only (unreadable to agents whose client keeps prompts for people); folding workflows into the guide (every agent loads all of them on connecting); one tool per workflow.

What would change it: clients that hand prompts to the model.
