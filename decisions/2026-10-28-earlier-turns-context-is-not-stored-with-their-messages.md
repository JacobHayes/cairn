# A turn's context is not stored with its message

Question (from the assistant's prompt-caching work): each turn's context (who the assistant works for, and the target as read) leads that turn's user message but is not stored, so the replayed history differs from what was sent, and the provider's cache misses from that message on. Should the context be stored with the message so earlier turns replay byte for byte?

Call: no. The cache still holds everything before the newest user message: the tools, the instructions, and every earlier message. Only the previous turn's user message, and what follows it, is written again.

Why not: a stored context would put a whole journey snapshot (kilobytes) beside every message, inside the conversation the person reads, the message count limit and the replay byte limit, so replay would trim sooner and the oldest turns' stale snapshots would be sent to the model as though they were current. It also needs a new field in the stored conversation and its migration. The gain is one message's worth of cache writes per turn.

Alternatives: store the context in a separate field and replay it; store only a digest.

What would change it: measured cache-write cost of the previous message dominating a turn's bill.
