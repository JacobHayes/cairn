# The engine names the current stages, the final milestone and the day a snooze lifts

Question (from removing the web app's copies of engine rules): the canvas decided which stages are current, the summary which milestone to lead with, and the plan list when a snooze lifts, each by re-reading the document in the tab. Should the engine derive them?

Call: yes. The derived values carry `current_stages` (top-level groups holding an acting-frontier or active node), `final_milestone` (the final milestone while it is open or done), and per node `snooze_lifts` (the latest day any hold on it or on a container above it ends; absent when a hold has no day). The web reads them. Completing the journey is also suggested once the final milestone is done, as before, but now only for a final milestone the engine considers in play, so a final milestone ruled out by an answer no longer suggests completion.

Authoring forms: the node form's pre-send checks (blank title or prompt, id and choice slug shapes, counts, byte limits, whole-number ranges) are the engine's. A wasm call takes the form's values for the fields the node has and returns one problem per field that cannot be sent, each with the field and plain wording. It reads each value through the types the patch reader builds a node from, so the form and the reader share one rule set; the wording of those types' errors is written for people. The form shows the messages at the fields, and nothing is sent until the engine has answered for the values as they are. The date-rule check and the condition check stay as they were.

Alternatives: reporting patch-reading failures as violations with a field (the reader's failures carry no field to place); keeping the three web computations.

What would change it: a field the form checks that the types do not, or a check that needs the whole graph (it belongs to the preview).
