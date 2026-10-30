# Tailscale trusts the proxies it lists

Question: Cairn behind an authenticating proxy (`tailscale serve`, or a reverse proxy on another machine) receives `Tailscale-*` identity headers. How does it trust the proxy without trusting anyone who can reach the listener?

Call: the provider takes a required list of trusted proxy addresses or networks and trusts the headers only from a peer inside it, an IPv4-mapped IPv6 address counting as its IPv4 address. A request carrying any `Tailscale-` header from an untrusted peer is refused (403), so a forged header never passes as a user or as anonymous; one with no such header is anonymous to this provider. From a trusted peer, a tagged node signs no one in (it is a machine), and a request with no login is anonymous, as the proxy's health check arrives.

Alternatives: treating forged headers as anonymous (hides a misconfigured or hostile client behind what looks like a missing sign-in); trusting `X-Forwarded-For` (the proxy is the peer that matters, and the chain is client-written); asking the local tailscaled who the peer is (a second way to do one job); implicitly trusting this machine (a listener reachable by others would need its own rule).

What would change it: a proxy without a stable source address, which would call for authenticating the proxy itself.
