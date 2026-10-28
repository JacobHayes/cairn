# The engine names the current stages, the final milestone and the day a snooze lifts

Question (from removing the web app's copies of engine rules): the canvas decided which stages are current, the summary which milestone to lead with, and the plan list when a snooze lifts, each by re-reading the document in the tab. Should the engine derive them?

Call: yes. The derived values carry `current_stages` (top-level groups holding an acting-frontier or active node), `final_milestone` (the final milestone while it is open or done), and per node `snooze_lifts` (the latest day any hold on it or on a container above it ends; absent when a hold has no day). The web reads them. Completing the journey is also suggested once the final milestone is done, as before, but now only for a final milestone the engine considers in play, so a final milestone ruled out by an answer no longer suggests completion.

Authoring forms: the pre-send checks in the node form (blank title or prompt, slug shapes, counts, byte limits) stay. The engine enforces each of them when it reads a patch, but it reports a violation there as an unreadable request with a location in the patch and developer wording, not as a message at the field, so the preview cannot yet drive the form's messages. The limits themselves are already read from the engine's generated tables.

Alternatives: a wasm call that returns a field problem like the condition check does (about ten files of plumbing and engine wording rewritten for people); leaving the three web computations.

What would change it: the engine reporting patch-reading failures as violations with the field and plain wording, at which point the form's checks can go.
