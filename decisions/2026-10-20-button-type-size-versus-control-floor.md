# Control labels are 14px; the 15px floor is for row, body and inspector text

Question: DESIGN's Buttons entry says Plex Sans 14px, while its accessibility section said rows, controls and inspector text are at least 15px. A UI critique of the index heads flagged every button and select (raised while shaping the shared index head).

Call: a control's own label (a button, a select) is the 14px of the Buttons entry, at the control height, which is what is built everywhere. The accessibility floor now covers row, body and inspector text and names that exception. DESIGN.md says so in one place.

Alternatives: raise buttons and selects to 15px at the same control height (touches every screen); leave the two statements contradicting.

What would change it: the owner asking for 15px controls, then one edit to the shared button and select styles and to both DESIGN entries.
