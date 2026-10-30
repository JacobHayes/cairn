# The configuration is one TOML file with a fixed set of environment overrides

Question: which values are deployment-specific, how an environment variable addresses one provider in a list, and what a public URL may be.

Call: `cairn.toml` holds the database path, listen address, public URL, time zone, a list of auth providers, and an optional assistant. The first four have no default, and a missing or unknown key fails startup, every problem listed at once. Environment overrides are a fixed table: one variable per top-level value and a secret variable per named provider; a secret variable naming no provider fails startup. The public URL is http or https at the origin's root, with no credentials, query, or fragment.

Alternatives: a generic mapping of every key to a variable (types guessed from strings, list entries addressed by index); defaults for the listen address or time zone (the zone is the deployment's choice); a path prefix in the public URL (the API and the app assume the origin's root).

What would change it: a proxy that can serve Cairn only under a path.
