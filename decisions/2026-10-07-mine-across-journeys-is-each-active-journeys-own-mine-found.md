# "Mine" across journeys is the union of each active journey's own "mine"

Question: the journey index's "mine" filter and the cross-journey "mine" list need the journeys where the caller holds something. The server can find journeys that refer to an identity, but that is a superset: a journey may name someone only in a condition or an answer.

Call: each active journey's own "mine" is projected in the tab, and a journey counts when it holds something of the caller's or the caller owns it. The cross-journey list is the unranked union. A completed or archived journey holds nothing of anyone's "mine".

Alternatives: the server's referencing filter alone (it lists journeys that give the caller nothing); a server-side "mine" filter on the index (the server would derive every journey on every index read, which the tab already does for the journeys it shows).

What would change it: deployments where many journeys refer to a person without giving them anything, making a server-side filter worth its derive cost.
