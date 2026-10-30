---
description: Record answers a person gives in conversation as decision answers, matching each decision's answer type.
---

# Record answers

1. **Match the value to the answer type.** `get_node` shows a decision's answer type. An
   answer is an `answer` mutation in `update_journey_state`, with the `decision` and one
   `value`:
   - `boolean`: true or false;
   - `single_choice`: one choice's id; `multi_choice`: a list of choice ids;
   - `text`: free text, possibly empty;
   - `date`: a calendar date, or a day with a time (`2026-10-10T08:15+01:00`, with the
     deployment's UTC offset at that time; a wrong offset is refused naming the right one);
   - `identity`: one identity key; `identity_list`: a list of identity keys.
2. **People are identities.** An identity answer names an identity's key. If the person named
   someone Cairn does not know, create them in the same patch: a `create_identity` with a new
   key, then the answer naming it. A write naming identities carries the
   `deployment_revision` the snapshot reported.
3. **Record why, when the person said.** An `answer` also takes a `rationale`
   (markdown): the reason this answer was given, in the person's terms. It belongs to
   that answer alone, so repeat it when you revise an answer for the same reason, and
   leave it off when the person gave none; never write one the person did not give.
4. **Batch what is already given.** Answers the person has given together go in one patch,
   an `answer` mutation each (one `patch_id`, the journey's current revision). Answers the
   person has not given yet wait for their question.
5. **Changing an answer** is another `answer`. Cairn re-evaluates what is relevant;
   work already done under the old answer is kept and reported if it no longer applies.
   The new answer has only the `rationale` you send with it: the previous reason is not
   carried over, and `get_node` shows the current one beside the answer.
6. **Say what it caused.** The write lists its consequences; tell the person what became
   relevant, what was dropped, and what is next.
