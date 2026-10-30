# The SSE write stall is enforced in the API and at the socket

Question: an SSE subscriber that cannot take a write for the stall is disconnected, but the server keeps handing frames to the kernel while its socket buffers have room, so a peer that stops reading or vanishes holds a subscriber slot for about a megabyte of ticks or the TCP retransmission timeout (about 15 minutes).

Call: the API drops a subscription when a send to its one-frame channel waits out the stall, which catches a stuck writer and a full peer. The listener sets `TCP_USER_TIMEOUT` to the stall, with keepalive, so unacknowledged data older than the stall closes the connection and the subscription goes at once; on a kernel without the option the binary runs and warns. The kernel's clock starts only when the peer's receive window closes, so the bound is measured from then: the stall, one coalescing interval, and the first zero-window probe (200 ms on Linux). The acceptance test runs the listener at a one-second user timeout through the same code, passed as a function argument rather than configuration, and checks the slot is freed within that bound and before the API's own stall could have freed it.

Alternatives: a per-connection timer in the API (it cannot see the kernel's buffers); smaller send buffers (a delay by size, not time); leaving it to retransmission (15 minutes per vanished peer); a full-length test (the mechanism does not depend on the length).

What would change it: a listener that cannot set the option (a non-Linux host, a unix socket behind a proxy), where the proxy's idle timeout must stand in.
