# The Store trait is generic, with Send futures, not object-safe

- Question: ARCHITECTURE asks for an async trait that names no runtime, with the composition root picking a backend. Async trait methods are not object-safe, and a multi-threaded server needs the futures to be `Send`.
- Call: `Store` (with `AuthStore` and `ConversationStore`, its supertraits) declares each method as returning `impl Future + Send`; the service layer (4.1) is generic over `S: Store`, and each composition root names its backend's type. No `async-trait` or boxing dependency. The Turso backend boxes its commit internally, since a commit's state machine is tens of kilobytes.
- Alternatives: `async-trait` boxing every call (a dependency and an allocation per call, for a choice made once per process); an enum over the two backends (the browser host would carry Turso).
- What would change it: a host that must choose among more backends at run time without generics.
