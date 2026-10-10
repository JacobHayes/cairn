-- B6: a container's snooze holds over its subtree, so a snooze until a node is rejected when
-- its target is in that subtree or depends on anything in it. A journey stored before the rule
-- was whole-subtree may hold a container snoozed until such a node, which the one rule now
-- refuses to load. Whether a target depends on the subtree is the engine's dependency graph
-- (explicit edges, conditions, inherited requirements, stage openings), which SQL cannot
-- evaluate, so every node snooze on a node with children is cleared, the rare valid one too: a
-- snooze is a hide, set again in one action. Date snoozes and snoozes on nodes without
-- children are untouched, as the rule never applied to them. Like the other migrations this
-- writes no event: an event needs an actor and a patch, and this one has neither.

DELETE FROM snoozes
WHERE until_node IS NOT NULL
  AND EXISTS (
    SELECT 1 FROM nodes AS child
    WHERE child.graph_id = snoozes.graph_id AND child.parent_key = snoozes.node
  );
