# A Tailscale login name is a verified email

Question: users meet their identities by verified email, and a provider lists only emails its issuer vouches for. Tailscale's login header gives a login name with no verified flag.

Call: a login name with an email's shape is listed as a verified email, and it is the login's subject. Every tailnet login was authenticated by the tailnet's own identity provider (Tailscale has no passwords), so it is as verified as that provider's email. A login that is not a real address (`someone@github`) matches no identity anyone would write.

Alternatives: no verified emails from Tailscale (a Tailscale-only deployment could never show anyone their own work); Tailscale's numeric user id as the subject (proxy headers do not carry it).

What would change it: a tailnet whose identity provider does not verify emails.
