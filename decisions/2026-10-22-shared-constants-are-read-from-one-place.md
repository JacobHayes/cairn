# Constants the app and the assistant shared are read from one place

Question: SSE timings in the client, the assistant's reserve for saving a turn, the request duration limit, and the rank terms' shares each existed in two places (audit of the authoring editors).

Call: the SSE coalescing interval and write stall are written to the generated rule table from the API crate's constants; the request duration limit moves to the schema crate's limits (the API re-exports it) so the assistant's turn reserve is that constant; the engine serves each rank term's share (`adds`) so the app no longer multiplies weights; the example configuration lists no rank values. No limit changed value, so no limit needed sign-off.

Alternatives: leave the literals with a comment naming the other copy.

What would change it: a limit that must differ between the app and the server.
