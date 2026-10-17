# A row says its date before what it needs

Question (from the Home and indexes work): a row carries one fact. When a node is late, near its
date, behind its plan or waiting on something, which does it say?

Call: one structured fact per row, ordered by weight: a conditional state; late (a decision says
"Decide by <date>, N days late"); behind; a start or due date within the next week (skipped when
the sort is gravity or unlocks, whose reason is the point of the list); what it needs; a dated
decision outside the soon window; then the sort's reason. The first applicable fact wins and
carries a tone (error for late and behind, warning for soon).

Alternatives: keep the sort's reason first (hides that a node is late behind "unlocks 4");
show several facts per row (crowds the two-line row, and the first still has to be chosen).

Would change it: viewers reading the second fact elsewhere, or a PRD requirement ranking dates
and needs differently.
