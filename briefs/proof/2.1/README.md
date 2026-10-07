# Proof for brief 2.1: Engine core: model, apply, state machines, events, replay

A route file now builds into a journey, and every change goes through one write path: a patch
either applies whole, advancing the revision by one with one event per change, or is rejected
with every problem listed by node path. Replaying the events alone rebuilds the same journey.

## The vendor evaluation, step by step

The vendor evaluation route builds into 25 nodes, 3 roles and 2 participation kinds. Its
scenario then runs eight patches:

| Step | What the patch does | Nodes changed | Events | Revision |
|---|---|---|---|---|
| 1 | Starts the journey and creates three people | 25 created | 4 | 1 |
| 2 | Answers the five up-front decisions | 5 decided | 5 | 2 |
| 3 | Reaches kickoff | `kickoff` reached | 1 | 3 |
| 5 | Finishes access; breaks the workload into two pieces | 1 done, 2 added | 3 | 5 |
| 8 | Finishes workload, baseline and findings; names a new reviewer | 6 done, 1 decided | 8 | 8 |

Replaying the 29 events of all 8 patches rebuilds records equal to what the patches produced.

## A rejected patch

One patch to the finished journey with five independent problems changes nothing and comes
back with all five, each with its code and the path of the node it is about:

| Path | Problem |
|---|---|
| `purpose` | a decided decision cannot be started |
| `decision-meeting` | the milestone is pinned by its date decision, not directly |
| `testing/partner-led` | the removal misses things the subtree holds |
| `setup/plan` | an explicit edge cannot join a node to its own child |
| `partner-runs` | the answer does not match the decision's answer type |

Values from the `walkthrough` example over the fixtures.
