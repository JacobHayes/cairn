# Consequences report what a patch unlocked and moved in or out of scope

Question: triage needs what an action unlocked, the save feed and agents want what moved in or out of scope, and a client that diffs the frontier before and after its write can credit someone else's concurrent change to its own action.

Call: a write's consequences carry, beside its warnings, what it newly put on the acting frontier, newly made not relevant, and newly made relevant or undecided. The server computes both sides at the same revision and the same today, so attribution is exact, and every transport carries them. The UI shows only the warnings at the moment of the edit; the rest feeds triage and the save feed, and agents receive both.

Alternatives: a frontier diff in the client (wrong under concurrency); a separate endpoint (a second request per write for what the write already knows).

What would change it: response size at the limits, which would cap and page these lists like other explanations.
