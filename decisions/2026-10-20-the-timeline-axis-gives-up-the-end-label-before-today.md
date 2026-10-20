# The Timeline axis gives up the end label before Today

**Question.** When the Today line is within a few days of the end anchor, the "Today" and "Ends" labels on the Timeline axis overlap, even with the end label cut to "Ends". The axis has two label rows and no room for a third. Which label gives way? (From the review of the derived-values work.)

**Call.** Today wins. The end label is the full words where they clear Today, "Ends" where only that clears it, and nothing where even that would overlap. The end line stays; its hover title carries the full words and the sentence under the axis ("Ends at ... on ...") already names the milestone and date.

**Alternatives.** A third label row (taller axis, shifts every row's alignment with the label track); dropping Today instead (Today is the label a person looks for first); stacking the end label above the ticks (collides with tick names).

**What would change it.** A taller axis designed for it, or Today and the end sharing one label ("Today, 3 days before the end").
