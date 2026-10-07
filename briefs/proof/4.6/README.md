# Proof for brief 4.6: Web client and app shell

Cairn has a web app. It runs against the server or entirely in the browser, derives each
journey in a worker, saves edits with their consequences shown, and keeps every open tab
current without a reload.

## Screens

The journey index, seeded from the fixtures, on the in-browser host:

![The journey index on the in-browser host](1-index-in-browser.png)

A journey derived in the worker; the derivation line names the revision and the day it was
derived for:

![A journey derived in the worker](2-journey-derived-in-worker.png)

A rename saved, with a notice of what it caused:

![The saved rename and its notice](3a-patch-saved-with-consequences.png)

The same journey in another tab, updated live:

![The other tab, updated live](3b-other-tab-updated-live.png)

Two tabs renaming the same node: the second save is not retried; the conflict is shown and
the draft kept:

![A conflict surfaced](4-conflict-surfaced.png)

A document from a newer engine: the tab stops deriving and saving and asks for a reload:

![The version-skew banner](5-version-skew-banner.png)

After the reload, the unsent rename is still there:

![The draft after the reload](6-draft-survives-reload.png)

The main flow on the server host, from the index to another tab's rename arriving live:
[main-flow.webm](main-flow.webm).

## Known limits

- Live updates of the journey index and of a person's own items come with those screens in
  a later brief.
