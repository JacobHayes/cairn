# The display-state words are the schema's, and the guide is held to them

Question (from the work making what agents are told agree with itself): the words a display
state is said by were a table in the web app, which a group's "ready" reads "not started" in,
while the served agent guide said every non-decision "ready" is "ready", so an agent and the
in-app assistant called a group ready while its chip said NOT STARTED. The app's list of
display states, which the Summary's counts follow, was a hand list too. Who owns the words?

Call: the schema. `DisplayState::word(kind)` holds the words, a kind's own word where it
differs (a decision is to decide, then decided; a group not started), and `DisplayState::ALL`
the order a count lists them in. Both are generated into the client's rule tables, which the
app's chips and counts read. The guide keeps its table as prose, and an integration test holds
its "Say" column to the words (the state's word, then each differing kind's), so the guide now
says a group not started is "not started". Badge tones stay in the app: they are a design
choice, not words.

Alternatives: generating the guide's table into `SKILL.md` (it is a published skill, written
by hand); serving the words through the API (no client but the app reads them).

What would change it: another surface needing words the schema does not hold, such as a
milestone's own word for done.
