# The page asks the derive worker for every engine call

Question (from cutting a tab's fixed memory cost): on the server host the page instantiated the engine a second time, beside the derive worker's, only to read and write route files. Which instance owns them?

Call: on the server host the derive worker holds the page's only engine instance; reading a route file and writing one are worker calls like derive and preview. Dropping the page's instance saves about 5 MB of a tab through a tour (median after a tour 158 MB against 163; first graph 129 against 135), with no time lost: the two calls are rare and already asynchronous where they are made. The in-browser host still instantiates the engine in the page, as its root is the service itself.

Alternatives: keep the page's instance (keeps about 5 MB per tab for two rare calls).

What would change it: a call the page must make synchronously, where waiting on the worker is not acceptable, would bring back an instance in the page.
