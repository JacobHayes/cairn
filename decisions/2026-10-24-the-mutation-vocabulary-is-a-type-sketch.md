# The mutation vocabulary is a type sketch

Question: spelled out as JSON Schema, the mutation vocabulary would be most of the tool list an agent loads on connecting. How is it spelled out so an agent can still write every structural change correctly?

Call: the tools that take a patch describe the mutations as compact type signatures generated from the same schema: one line per `op` with its fields and the first sentence of its description, then each shared type once, then the key prefixes. Required, optional and nullable fields read differently, so a required field that takes `null` as a value still shows it, and a node's per-kind rules sit in a comment beside its type. The generator supports exactly the constructs the schemas use and panics on any other, so a new construct fails the tool-listing tests instead of producing a wrong sketch. Parsing stays by the typed schema, as strict, and a mismatch names its path. The sketch is held to a byte limit. An open `{op}` object does not suit a provider's strict structured-output mode, which no adapter uses.

Alternatives: compacted JSON Schema (about four times the size); serving the schemas from a read tool (a call before the first structural write); loose objects with no description (agents guess shapes and are refused).

What would change it: a model that misreads the sketch in practice, or a provider adapter that needs strict mode.
