# A tool says what it writes, and the assistant's tool list follows

Question (from the work making what agents are told agree with itself): the assistant kept its
own lists of tool names, the ones it refuses, beside the tool set that defines them. Where do
these live?

Call: each tool's entry says what it writes, as one of five kinds: nothing, a patch drafted
from its arguments, a patch built from a file the caller hands it, a proposal, or a
proposal's confirmation (applying it). The endpoint's read-only and idempotent hints and the
assistant both read it. The assistant offers every tool except the file and confirmation
kinds (the user's to do), and refuses a call to any other name. A new kind is a compile error
where the assistant builds its list until it is decided.

Alternatives: a list of refused names beside the tool set (two homes kept in step).

What would change it: a write the assistant should run that fits none of the kinds, such as
one whose patch only the service can work out but which is safe to apply directly.
