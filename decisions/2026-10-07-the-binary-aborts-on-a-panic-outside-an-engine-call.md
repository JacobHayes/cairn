# The binary aborts on a panic outside an engine call, and never panics on a log it cannot write

Question: a panic outside the engine must stop the process, but tokio catches a panic in a spawned task and carries on, and the service must catch engine panics to answer 500, so the process cannot simply be built with `panic = "abort"`.

Call: the service marks the thread while it runs an engine call, and the binary's panic hook leaves those panics to the service and logs and aborts on every other. tracing-subscriber's internal error report is off: it prints to standard error, which panics when standard error is closed.

Alternatives: `panic = "abort"` (loses the engine-panic 500); catching panics per request (shared state may be inconsistent).

What would change it: engine calls that run on threads other than the one that entered them.
