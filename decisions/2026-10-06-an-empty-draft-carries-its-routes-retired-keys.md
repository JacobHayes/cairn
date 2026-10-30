# An empty draft carries its route's retired keys, and publishing retires what it left out

Question: a draft opened by an import or a saved journey starts empty, so without care it would hold none of the route's retired keys, and nothing would retire the keys its content leaves out. A key could come back.

Call: a draft that starts empty carries the latest version's retired keys, so adding one is refused. Publishing retires every node, role and kind key of the version the draft extends that the draft leaves out. An import that supplies a retired key is refused, and so is a save as route of a journey behind its route that would bring one back, each naming the key and what to do instead.

Alternatives: retire at open time (an import's matched keys would then be refused).

What would change it: a store that keeps a route's retired keys outside its version graphs.
