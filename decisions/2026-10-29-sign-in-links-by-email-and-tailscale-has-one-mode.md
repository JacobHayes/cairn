# A new login joins the user with its verified email, and Tailscale has one mode

Question: how are several logins tied to one user, and how does the Tailscale provider identify a person?

Call: a new login whose verified email another user's login holds joins that user; otherwise it creates one. The rule is always on, with no setting, and an email held by two users joins neither. A login joins a user by this rule alone, with no per-provider switch. A session cookie that names no live session is ignored and cleared, not refused. The Tailscale provider reads identity from a trusted proxy only, with a required `trusted_proxies`; on a tailnet-only host, `tailscale serve` with `trusted_proxies = ["127.0.0.1"]` covers it.

Alternatives: a step that joins a second login to the signed-in user (the email rule covers the common case); asking tailscaled directly over its socket (no deployment runs it, and the proxy covers the same host); a per-provider switch for matching by email (the rule is safe only for verified emails, which every provider already gives).

What would change it: a deployment that needs one user across logins whose emails differ.
