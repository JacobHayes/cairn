# Behind Google Cloud IAP, Cairn verifies the signed assertion on every request

Question: a deployment behind Google Cloud's Identity-Aware Proxy needs each person signed in as themselves, without a second sign-in after IAP's or a header-rewriting proxy in front of Cairn.

Call: a `gcp_iap` provider verifies the JWT IAP signs onto every request, with the same ID-token verifier the OIDC provider uses, so nothing is checked by hand: Google's IAP key set, the issuer, the configured audience, expiry, and issue time, ES256 only. The subject is IAP's stable account id, and a Google account's email is listed as verified (`decisions/2026-10-21-behind-iap-only-a-google-accounts-email-is-verified.md`). A missing header is absent; a bad one is a refused credential. Since the signature is the credential, the provider runs on any listener. The key set ages out and is refetched (`decisions/2026-10-20-iap-keys-age-out-and-failed-fetches-hold-off-the-next.md`); fetches are rate-limited, so forged key ids cannot cause one per request.

Alternatives: trusting IAP's plain email header (safe only while nothing but the load balancer can reach the port; Google's guidance is to verify the JWT); the email as the subject (a renamed account would become a new identity).

What would change it: IAP signing with another algorithm or moving its key set.
