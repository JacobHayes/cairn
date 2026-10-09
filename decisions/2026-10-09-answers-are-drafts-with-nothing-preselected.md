# Answers are drafts with nothing preselected, and a new answer starts with no reason

Question (from the web redesign, PRD B2, C8): the answer editor sat behind a fold and an Answer button, a boolean preselected "yes", and a rationale could be carried forward silently to a different answer, the bug the rationale work plants.

Call: the form is the display. A decision shows its context, question, help, and input together; picking an option is a draft, nothing is preselected, and Save (Cmd or Ctrl+Enter) is enabled only when the answer or reason differs from what is stored. Drafts survive a reload. The reason field is open and optional; choosing a different answer empties it and offers the previous reason with Reuse, so copying it is deliberate. Editing only the reason of the current answer opens it with the current text. If someone else answers mid-draft, a callout replaces Save with Use mine and Discard mine.

Alternatives: a preselected default (hides that a decision is open, which the PRD's Later rejects for default answers); carrying the reason forward (silently attaches a reason to an answer it was not written for).

What would change it: decisions whose answers are usually refinements of the last one, where carrying the reason as an editable starting point would serve better.
