# A write is an operation the edit manager owns

Question: every write path in the web app has one owner. What does a write's answer make of it, what is resent, and how does the control that sent it see it?

Call: every write a control starts (a patch, a proposal write, a route import) goes to the edit manager under a stable edit key made from what the control is, never from `useId()`, so a control shown again finds its write and a rejection survives leaving the screen. An edit has at most one busy write; a new one replaces a settled one.

- Answers: applied lands it; a rejection is `rejected` and stays until discarded or replaced; a refusal saying the subject no longer exists is `gone`; a 4xx to the first send is `stopped`. No answer, or a 4xx to a resend, is `unconfirmed`: it resends the same patch and id on the shared backoff up to a limit, then asks for Retry. Calling every failure unconfirmed would resend definite refusals.
- A pre-send engine check is the write's first phase, not a step before it.
- Text typed after a send is never lost: a form closes on landing only if its draft is unchanged since it sent; otherwise it keeps the unsent text. A node's action forms are one machine, so only one is open.

Alternatives: a hook per path with its own rejection state (rejections lost on navigation); a draft cleared on any landing.

What would change it: a host that cannot tell a refusal from silence.
