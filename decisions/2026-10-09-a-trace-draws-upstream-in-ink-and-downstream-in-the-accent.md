# A trace draws upstream in ink and downstream in the accent

Question (from the web redesign, PRD C7): trace was a separate toggle, and the redesign needed upstream and downstream told apart without adding a hue to an always-on interaction.

Call: selecting a node traces it, always the full chain. Upstream ("needs") is ink at 1.5px, with finished upstream lines in border grey so what is still in the way stands out; downstream ("unblocks", including nodes whose relevance it decides) is the accent at 1.5px. Each traced card hangs a Needs or Unblocks tag, so colour is never the only signal. Everything else fades to one level (0.3), and fades never multiply; with a selection the trace decides what is full strength over any filter. Hovering a card does nothing on the canvas beyond the hover tint and a tooltip; hovering an edge draws it and its ends in ink. The trace is off during proposal review, so a diff and a trace never show at once.

Alternatives: a new categorical colour for upstream (an always-on hue in a palette meant for rare use); a one-hop hover preview (a third highlight layer on top of the trace and the fades).

What would change it: users misreading the accent downstream as "selected", which would call for a different upstream and downstream pair.
