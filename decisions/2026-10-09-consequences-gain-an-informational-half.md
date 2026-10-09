# Consequences report what a patch unlocked and moved in or out of scope

Question (from the web redesign, PRD D7, C11, I5): consequences reported only warnings. Pass order needs what an action unlocked, the sync chip's Recent and agents want what moved in or out of scope, and a client that diffs the frontier before and after can credit another person's concurrent change to its own action.

Call: consequences gain three informational lists, never warnings: `unlocked` (newly on the acting frontier), `out_of_scope` (newly not relevant), and `into_scope` (newly relevant or undecided). The server computes both sides at the same revision and today, so attribution is exact, and every transport carries them. The UI shows only warnings at the moment of the edit; the informational half feeds pass order and Recent, and agents receive both.

Alternatives: a client-side frontier diff (wrong under concurrency); a separate endpoint (a second request per write for what the write already knows).

What would change it: response size at the limits, which would cap and page the lists like other explanations.
