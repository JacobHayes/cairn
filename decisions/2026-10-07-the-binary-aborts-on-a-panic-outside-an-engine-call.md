# The binary aborts on a panic outside an engine call, and never panics on a log it cannot write

- Question: PRACTICES says a panic outside the engine stops the process, but tokio catches a panic in a spawned task (every connection and the SSE pumps) and carries on; the service catches engine panics with `catch_unwind`, so the process cannot simply be built with `panic = "abort"`.
- Call: the service marks the thread while it runs an engine call (`cairn_service::engine_call_active`), and the binary's panic hook leaves those to the service (the request fails with a 500, logged and counted by the API) and logs and aborts on every other panic. tracing-subscriber's internal error report is off, since it `eprintln!`s, which panics when standard error is closed: a server whose log reader went away kept answering in testing only with it off.
- Alternatives: `panic = "abort"` (loses the engine-panic 500); catching panics per request (contrary to PRACTICES: shared state may be inconsistent).
- What would change it: engine calls that run on other threads than the one that entered them.
