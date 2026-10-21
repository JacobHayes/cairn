# A tool says what it writes, and the assistant decides by that

Question (from the work making what agents are told agree with itself): the assistant's write
wrapper kept its own lists of tool names, the proposal tools that pass through and the tools it
refuses, beside the tool set that defines them, and read and built tool arguments and answers
by raw field names. The system prompt listed which changes are state and which structural by
hand, beside the engine's own classification. Where do these live?

Call: each tool's entry says what it writes, as one of five kinds: nothing, a patch drafted
from its arguments, a patch built from a file the caller hands it, a proposal, or a
proposal's confirmation (applying it). The endpoint's read-only and idempotent hints and the
wrapper both read it. The wrapper refuses the file and confirmation kinds (the user's to do),
passes proposals through, and decides patch tools by the patch they would submit, as before;
a new kind is a compile error in the wrapper until it is decided. The tool set exposes its
proposal and write answers and the proposal tools' arguments as types, and runs a proposal
create or edit for the assistant from them, so the wrapper names no field. The prompt says
each change class's meaning from the class itself, and the refused writes from the refusal
reasons the wrapper gives.

Alternatives: exposing only the dispatch group (a group mixes importing with patch tools and
applying with drafting, so the names would still be listed); a test that the lists match the
tool set (two homes kept in step).

What would change it: a write the assistant should run that fits none of the kinds, such as
one whose patch only the service can work out but which is safe to apply directly.
