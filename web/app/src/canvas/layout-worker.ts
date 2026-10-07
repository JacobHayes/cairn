// C15: the layout worker (ARCHITECTURE, Web UI: Canvas: ELK in a web worker), so laying out a
// dense journey never blocks input. ELK's worker build, loaded in a worker's scope, answers
// ELK's own messages (`{ cmd: "layout", graph, id }` in, the laid-out graph or an error out);
// the page talks to it through ELK's API (layouts.ts), so the protocol is ELK's, not ours.
import "elkjs/lib/elk-worker.min.js";
