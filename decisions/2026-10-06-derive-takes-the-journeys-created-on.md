# Derive takes the journey's `created_on`

- Question: rules may measure from `journey.created_at` (A8), a fact the execution layer fixes, but `derive(graph, inputs)` sees only the graph document, and the creation day lives on the journey's header.
- Call: `derive(journey, created_on: Option<Date>, inputs)`; a route derives with none, so `created_at` rules give no bound there.
- Alternatives: a field on `DeriveInputs` (inputs are host-supplied context, and the domain document already carries the header beside them, so the day would be stated twice); copying it into the graph's state (a second stored copy of a header field).
- What would change it: other header fields that derive needs, which would argue for passing the journey whole.
