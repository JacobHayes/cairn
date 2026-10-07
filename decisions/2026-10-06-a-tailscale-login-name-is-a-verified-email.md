# A Tailscale login name is a verified email

- Question: H3 matches users to entities by verified email, and a provider lists only emails its issuer marks verified. Tailscale's whois and its `Tailscale-User-Login` header give a login name, with no verified flag.
- Call: the login name is listed as a verified email when it has an email's shape. Every tailnet login was authenticated by the tailnet's identity provider (Tailscale has no passwords of its own), so the login is as verified as that provider's email; a login that is not an address in practice (`someone@github`) matches no entity email anyone would write. A tagged node (a machine, not a person) signs no one in. The identity's subject is the login name in both modes, so direct and proxy mode name the same identity.
- Alternatives: no verified emails from Tailscale (a Tailscale-only deployment would have no "mine", H3); Tailscale's numeric user id as the subject (proxy mode's headers do not carry it).
- What would change it: a tailnet whose identity provider does not verify emails, which would then need this provider's verified emails off.
