---
description: Record answers a person gives in conversation as decision answers, matching each decision's answer type.
---

# Record answers

1. **Match the value to the answer type.** `get_node` shows a decision's answer type.
   `answer_decision` takes one `value`:
   - `boolean`: true or false;
   - `single_choice`: one choice's id; `multi_choice`: a list of choice ids;
   - `text`: free text, possibly empty;
   - `date`: a calendar date;
   - `entity`: one entity key; `entity_list`: a list of entity keys.
2. **People are entities.** An entity answer names an entity's key. If the person named
   someone Cairn does not know, create them first with `manage_entity` (`create`), or ask.
   A write naming entities carries the `deployment_revision` the snapshot reported.
3. **One answer per write.** Each `answer_decision` is one patch with its own `patch_id`,
   against the journey's current revision. Several answers given at once can go in one
   `apply_patch` with an `answer` mutation each.
4. **Changing an answer** is another `answer_decision`. Cairn re-evaluates what is relevant;
   work already done under the old answer is kept and reported if it no longer applies.
5. **Say what it caused.** The write lists its consequences; tell the person what became
   relevant, what was dropped, and what is next.
