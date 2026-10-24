# Date chains are compact and held to the explanation limit

- Question: the work cutting the MCP context cost found each date-chain constraint at about 58 tokens of nested objects, and a node's chains unbounded (a node at the node limit answers up to 2,000 constraints per chain), unlike every other explanation list. What shape do chains take, for whom, and how are they bounded?
- Call:
  - **One compact shape everywhere.** A constraint is `after`, `before`, `days`, `source` and `conditional` when it holds only under a condition. An instant and a source are each one string (`n_x.start`, `created`; `dependency:n_a:n_b:inherited:n_up`) that names every node the old nested form named, so nothing is lost: the kind, how a dependency arose, the node responsible and conditionality all stay. The schema has one chain type, so the server, the agent tools, the rejections and the app's local derive all use it; the app parses the strings where it says a constraint in words.
  - **Server responses cut chains.** Node detail (HTTP and MCP) carries at most the explanation entry limit of constraints and of fixed dates per chain (earliest start, latest start, due, shortfall) with a `total`, and the explanations read pages the rest in chain order, from a cursor, so the pages together are the whole chain. The app's local derive keeps whole chains: the app renders them and holds the whole journey.
  - **A rejection's chains are not cut.** They explain a patch that was refused, which no later read can continue; they stay bounded by the limit on contradictory chains per rejection and the node limit.
  - **No new limit.** The existing explanation entry limit is the cap.
- Alternatives:
  - Compact only in the agent tools: the agent's schema would differ from the server's and the app's, and the HTTP node detail would keep the large form.
  - Drop the node fields a source's ends already imply: the ends do not always name the responsible node (an inherited requirement, a condition gate, a stage opening), and a form that cannot be read back is a second type to keep true.
  - Cut rejection chains too: a refused write's chain could not be continued by any read.
- What would change it: a client that needs the nested form (it would parse the string); a chain whose fixed dates grow past the limit.
