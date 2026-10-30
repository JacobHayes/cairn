# Navigations are urgent updates, not transitions

Question: a router that renders each navigation as a React transition leaves the page showing the old address for a moment, and a click or key in that window acts on it (Escape then `v` switches the view of the node just closed); on a loaded machine browser tests hit that window. Should handlers read the live address, or should the address stop lagging?

Call: the router runs without transitions (`useTransitions={false}`), so a navigation renders before the next input is handled and every handler that derives the next address from the current one sees the move. The cost is that a first visit to a screen whose code is still loading shows the shell's loading state instead of holding the previous screen.

Alternatives: each handler reading `window.location` at event time (a second source of truth for the address in every handler); tests waiting for the visible result before acting (hides the bug from tests and leaves it for people).

What would change it: a screen that must hold the previous one while its code loads (preload it, or wrap only that navigation in a transition).
