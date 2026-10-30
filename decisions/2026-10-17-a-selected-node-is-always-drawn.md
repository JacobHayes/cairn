# A selected node is always drawn, and the address stays as opened

Question: every node has its own address, and a link to a node must show it, but the query holds only what someone chose (depth, filter, search, collapsed containers). Who makes a hidden node appear, and may it rewrite the address?

Call: the engine's level query takes the selected node, which is drawn whatever would hide it, with every container above it open; everything else follows the usual rules. The address is never rewritten to reveal it. The client reads the plain level first and asks again with the selection only when the plain level leaves it out. The list and timeline follow the same rule. Collapsing a container that holds the selection selects the container.

Alternatives: writing the reveal into the address (a lowered depth, a cleared search: long links that misstate what was chosen and leave a stale reveal behind); drawing the selection in the client only (stand-ins, edges and the hidden-prerequisites mark would disagree with the engine).

What would change it: a level large enough that the second read is noticeable.
