# IAP keys age out, failed fetches hold off the next, and assertions last at most IAP's lifetime

**Question.** How long does Cairn trust Google's IAP signing keys and an assertion, and what happens while Google's key endpoint is down?

**Call.**
- The kept key set is trusted only up to a fixed age. Past that the next request fetches it again; if that fetch fails, requests are refused as unavailable rather than verified against the aged set, so a withdrawn key stops verifying.
- Every fetch, failed or not, holds off the next for the refetch interval. One fetch runs at a time on its own task, and a request whose key is kept never waits on it. A clock stepped back counts as long ago.
- An assertion's lifetime from issue to expiry is held to Google's guidance, plus the skew at each end.

**Alternatives.** Keep verifying against the aged set while a refresh fails (fails open exactly when a withdrawn key matters); honour the response's `Cache-Control` (parsing for no documented rotation period); refetch only on an unknown key id (a withdrawn key stays trusted forever).

**What would change it.** Google documenting a shorter withdrawal window or another lifetime, or a deployment that must sign in through a longer outage at Google's key endpoint.
