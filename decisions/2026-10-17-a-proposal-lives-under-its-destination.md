# A proposal lives under its destination

Question: a proposal reachable only at an address with no destination in it leaves a journey, a route or Home no way to say one is waiting. Where does a proposal live?

Call: a proposal has one destination (a journey, a route or its draft, or the deployment; possibly one not yet created), and every app and API address of it sits under that destination. There is no top-level proposal page, alias or redirect. A proposal asked for under a destination that is not its own is not found, before any write. A listing endpoint outside any one destination serves only what no single destination holds: proposals waiting on the caller, and ones that would create a journey or route. MCP tools name a proposal by id alone, since ids are unique and an agent's arguments name ids, not addresses.

Alternatives: id-only addresses with lists beside them (two homes, and the page would not know which section to light); a top-level Proposals screen (a place to go looking, away from what the proposal is about); the destination as an optional query parameter (it can disagree with the proposal).

What would change it: a proposal whose destination can change, or one spanning several destinations.
