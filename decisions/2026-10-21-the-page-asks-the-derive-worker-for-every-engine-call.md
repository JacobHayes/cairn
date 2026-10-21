# The page asks the derive worker for every engine call

Question (from cutting a tab's fixed memory cost): on the server host the page instantiated the engine a second time, beside the derive worker's, only to read and write route files and to decide whether a stale write is safe to resubmit. The safe retry's rule was also written twice, in the Rust client and in the TypeScript client. Which copy owns each?

Call: on the server host the derive worker holds the page's only engine instance; reading a route file, writing one, and rebasing a stale patch are worker calls like derive and preview. Dropping the page's instance saves about 5 MB of a tab through a tour (median after a tour 158 MB against 163; first graph 129 against 135), with no time lost: the three calls are rare and already asynchronous where they are made. The safe retry's rule (when a stale patch may be resubmitted, and with which revisions) lives once, in the schema crate as `Patch::rebased`: the Rust client calls it, and the browser asks it of the worker through the wasm module. The TypeScript client keeps only the loop: send, ask the rule, resend up to the bound, stop on version skew. The in-browser host still instantiates the engine in the page, as its root is the service itself.

The two copies differed in one corner: for a patch that targets a proposal, the TypeScript copy never rebased, while the Rust one rebases when only the revisions the patch names moved. No page sends such a patch through the safe retry (proposal writes are sent once and never rebased), so the Rust rule is now the only one with no change anyone can see.

Alternatives: keep the page's instance and drop only the duplicate rule (keeps about 5 MB per tab for three rare calls); keep the rule in TypeScript and drop the Rust copy (the Rust clients and the testbed need it without a browser).

What would change it: a call the page must make synchronously, where waiting on the worker is not acceptable, would bring back an instance in the page.
