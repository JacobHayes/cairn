//! Constraint chains (F2, F3, F5, F6): the one shape every date relationship takes, "B is
//! at least k days after A", and the chains of them that explain a bound or make a plan
//! impossible. The chain that explains a due date and the chain that makes a plan
//! contradictory are the same kind of thing, so both use these types.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::collections::{BoundedVec, ChainCountPerRejection};
use crate::id::NodeKey;
use crate::patch::Mutation;
use jiff::civil::Date;

// PRD: F2.
/// Which instant of a node: work has a start and a finish; a decision or milestone has
/// one instant, which is both.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum InstantPoint {
    /// When it starts.
    Start,
    /// When it finishes.
    Finish,
}

// ARCHITECTURE: Date network.
/// A point in time the date network solves for. A container's
/// entry is internal and never shown, so it has no form here.
///
/// On the wire it is one short string: `n_x.start` or `n_x.finish` for a node's start or
/// finish, `n_x.answer` for a date decision's answer, `created` for the journey's creation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Instant {
    /// A node's start or finish.
    Node {
        /// The node.
        node: NodeKey,
        /// Which instant.
        point: InstantPoint,
    },
    /// The journey's `created_at`.
    CreatedAt,
    // PRD: A8, F2.
    /// A date decision's answer, a date rule's source. It is the answered date, apart
    /// from the decision's own instant, which is when it was decided.
    Answer {
        /// The decision.
        decision: NodeKey,
    },
}

impl fmt::Display for Instant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node {
                node,
                point: InstantPoint::Start,
            } => write!(formatter, "{node}.start"),
            Self::Node {
                node,
                point: InstantPoint::Finish,
            } => write!(formatter, "{node}.finish"),
            Self::CreatedAt => formatter.write_str("created"),
            Self::Answer { decision } => write!(formatter, "{decision}.answer"),
        }
    }
}

impl FromStr for Instant {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        if text == "created" {
            return Ok(Self::CreatedAt);
        }
        let bad = || {
            format!(
                "{text:?} is not an instant: `n_x.start`, `n_x.finish`, `n_x.answer` or `created`"
            )
        };
        let (node, point) = text.rsplit_once('.').ok_or_else(bad)?;
        let node: NodeKey = node.parse().map_err(|_| bad())?;
        match point {
            "start" => Ok(Self::Node {
                node,
                point: InstantPoint::Start,
            }),
            "finish" => Ok(Self::Node {
                node,
                point: InstantPoint::Finish,
            }),
            "answer" => Ok(Self::Answer { decision: node }),
            _ => Err(bad()),
        }
    }
}

crate::serde_util::text_serde!(
    Instant,
    "Instant",
    "A point in time: `n_x.start` or `n_x.finish` for a node's start or finish, `n_x.answer` for a date decision's answered date, `created` for the journey's creation.",
    Some(r"^(created|n_[a-z0-9][a-z0-9_-]*\.(start|finish|answer))$")
);

// PRD: glossary, Condition gate / implicit edge.
/// How a dependency arose.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DependencyVia {
    /// An explicit `requires` edge.
    Explicit,
    /// A parent waiting on its child (Containment).
    Containment,
    /// An ancestor's requirement, inherited.
    Inherited {
        /// The ancestor whose requirement applies.
        ancestor: NodeKey,
    },
    /// A condition referencing a decision (Gating).
    Condition {
        /// The node whose condition creates the gate.
        condition_on: NodeKey,
    },
}

// PRD: F2, F7.
/// Where a constraint comes from: what to edit to change it.
///
/// On the wire it is one short string of the kind and the nodes it names, separated by
/// colons: `due_by:n_a`, `not_before:n_a`, `estimate:n_a`,
/// `containment:n_parent:n_child`, and `dependency:n_a:n_b:` then how it arose
/// (`explicit`, `containment`, `inherited:n_ancestor`, `condition:n_decision`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConstraintSource {
    /// A node's `due_by` rule.
    DueBy {
        /// The node.
        node: NodeKey,
    },
    /// A node's `not_before` rule.
    NotBefore {
        /// The node.
        node: NodeKey,
    },
    /// A dependency: the dependent starts after the requirement finishes.
    Dependency {
        /// The dependent.
        node: NodeKey,
        /// The requirement.
        requires: NodeKey,
        /// How the dependency arose.
        via: DependencyVia,
    },
    /// An estimate: a node finishes at least its estimate after it starts.
    Estimate {
        /// The node.
        node: NodeKey,
    },
    /// Containment: a child finishes before its parent's own work starts.
    Containment {
        /// The parent.
        parent: NodeKey,
        /// The child.
        child: NodeKey,
    },
}

impl fmt::Display for ConstraintSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DueBy { node } => write!(formatter, "due_by:{node}"),
            Self::NotBefore { node } => write!(formatter, "not_before:{node}"),
            Self::Estimate { node } => write!(formatter, "estimate:{node}"),
            Self::Containment { parent, child } => {
                write!(formatter, "containment:{parent}:{child}")
            }
            Self::Dependency {
                node,
                requires,
                via,
            } => {
                write!(formatter, "dependency:{node}:{requires}:")?;
                match via {
                    DependencyVia::Explicit => formatter.write_str("explicit"),
                    DependencyVia::Containment => formatter.write_str("containment"),
                    DependencyVia::Inherited { ancestor } => {
                        write!(formatter, "inherited:{ancestor}")
                    }
                    DependencyVia::Condition { condition_on } => {
                        write!(formatter, "condition:{condition_on}")
                    }
                }
            }
        }
    }
}

impl FromStr for ConstraintSource {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let bad = || format!("{text:?} is not a constraint source");
        let key = |part: &str| part.parse::<NodeKey>().map_err(|_| bad());
        let parts: Vec<&str> = text.split(':').collect();
        match parts.as_slice() {
            ["due_by", node] => Ok(Self::DueBy { node: key(node)? }),
            ["not_before", node] => Ok(Self::NotBefore { node: key(node)? }),
            ["estimate", node] => Ok(Self::Estimate { node: key(node)? }),
            ["containment", parent, child] => Ok(Self::Containment {
                parent: key(parent)?,
                child: key(child)?,
            }),
            ["dependency", node, requires, via @ ..] => {
                let via = match via {
                    ["explicit"] => DependencyVia::Explicit,
                    ["containment"] => DependencyVia::Containment,
                    ["inherited", ancestor] => DependencyVia::Inherited {
                        ancestor: key(ancestor)?,
                    },
                    ["condition", on] => DependencyVia::Condition {
                        condition_on: key(on)?,
                    },
                    _ => return Err(bad()),
                };
                Ok(Self::Dependency {
                    node: key(node)?,
                    requires: key(requires)?,
                    via,
                })
            }
            _ => Err(bad()),
        }
    }
}

crate::serde_util::text_serde!(
    ConstraintSource,
    "ConstraintSource",
    "Where a constraint comes from, the kind then the nodes it names, colon-separated: `due_by:n_a`, `not_before:n_a`, `estimate:n_a`, `containment:n_parent:n_child`, or `dependency:n_a:n_b:` then `explicit`, `containment`, `inherited:n_ancestor`, or `condition:n_decision`.",
    None::<&str>
);

/// One constraint: `after` is at least `offset_days` after `before` (offsets may be zero or
/// negative).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Constraint {
    /// The earlier instant.
    pub before: Instant,
    /// The later instant.
    pub after: Instant,
    /// The least number of days between them.
    #[serde(rename = "days")]
    pub offset_days: i32,
    /// Where it comes from.
    pub source: ConstraintSource,
    // PRD: F2.
    /// It passes through an undecided node, so it may not apply.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub conditional: bool,
}

// PRD: F1, F3, F6.
/// What fixes an instant on a chain: a pin, an actual date, a date answer, or today. A chain always ends at one of these.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FixedBy {
    // PRD: F5.
    /// A pin, which a resolution may move.
    Pin,
    // PRD: F6.
    /// An actual date, a fact that is never moved: a recorded start or finish, or the
    /// journey's `created_at`.
    Actual,
    // PRD: A8.
    /// A date decision's answer, which a revised answer moves.
    Answer,
    // PRD: F3.
    /// Today, which holds unfinished work back: it starts, or finishes, no earlier.
    Today,
}

/// A date fixed on a chain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FixedDate {
    /// The instant.
    pub instant: Instant,
    /// The date.
    pub date: Date,
    /// A pin or an actual.
    pub fixed_by: FixedBy,
}

/// A chain of constraints, with the dates fixed on it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Chain {
    /// The constraints, in order along the chain.
    pub constraints: Vec<Constraint>,
    /// The pins and actuals on it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed: Vec<FixedDate>,
    /// How many constraints and fixed dates the whole chain holds, when a server response
    /// cut it to `explanation_entry_count_max` of each; absent for a chain given whole. The
    /// rest pages through the explanations of the value the chain explains.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub total: u32,
}

impl Chain {
    // ARCHITECTURE: Read path.
    /// The chain a server response carries: at most `explanation_entry_count_max`
    /// constraints and as many fixed dates, in chain order, with the total of both when
    /// either was cut.
    #[must_use]
    pub fn for_response(mut self) -> Self {
        let limit =
            usize::try_from(crate::limits::EXPLANATION_ENTRY_COUNT_MAX).unwrap_or(usize::MAX);
        if self.constraints.len() <= limit && self.fixed.len() <= limit {
            return self;
        }
        self.total = u32::try_from(self.constraints.len() + self.fixed.len()).unwrap_or(u32::MAX);
        self.constraints.truncate(limit);
        self.fixed.truncate(limit);
        self
    }
}

// PRD: F5, F6.
/// A chain that requires a date to precede itself, or a plan reality can no longer
/// meet, with how many days it is short and the moves that would resolve it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShortChain {
    /// The chain.
    pub chain: Chain,
    /// Days short.
    pub shortfall_days: u32,
    // PRD: F5, F6.
    /// The resolution moves: each an ordinary mutation that, applied alone, gives
    /// this chain the days it lacks (shift, repin, or unpin a pin on it; revise the answer
    /// behind a pin; adjust a rule's offset; revise an estimate; drop an explicit
    /// requirement). Nothing applies them automatically.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolutions: Vec<Mutation>,
}

// PRD: F5.
/// The contradictory chains a rejection lists: at most
/// `chain_count_per_rejection_max`, and whether there were more.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChainList {
    /// The chains found, up to the limit.
    pub chains: BoundedVec<ShortChain, ChainCountPerRejection>,
    /// The search was cut short at the limit.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub more: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(text: &str) -> NodeKey {
        text.parse().unwrap()
    }

    /// Every instant and every constraint source, in whatever way it arose, is written as one
    /// short string and read back as itself, naming the same nodes.
    #[test]
    fn instants_and_sources_round_trip_through_their_text() {
        let instants = [
            Instant::Node {
                node: node("n_a-1"),
                point: InstantPoint::Start,
            },
            Instant::Node {
                node: node("n_a"),
                point: InstantPoint::Finish,
            },
            Instant::Answer {
                decision: node("n_when"),
            },
            Instant::CreatedAt,
        ];
        for instant in instants {
            let text = serde_json::to_string(&instant).unwrap();
            assert!(text.starts_with('"'), "{text} is a string");
            assert_eq!(serde_json::from_str::<Instant>(&text).unwrap(), instant);
        }
        let vias = [
            DependencyVia::Explicit,
            DependencyVia::Containment,
            DependencyVia::Inherited {
                ancestor: node("n_up"),
            },
            DependencyVia::Condition {
                condition_on: node("n_gate"),
            },
        ];
        let mut sources = vec![
            ConstraintSource::DueBy { node: node("n_a") },
            ConstraintSource::NotBefore { node: node("n_a") },
            ConstraintSource::Estimate { node: node("n_a") },
            ConstraintSource::Containment {
                parent: node("n_p"),
                child: node("n_c"),
            },
        ];
        sources.extend(vias.into_iter().map(|via| ConstraintSource::Dependency {
            node: node("n_a"),
            requires: node("n_b"),
            via,
        }));
        for source in sources {
            let text = serde_json::to_string(&source).unwrap();
            assert_eq!(
                serde_json::from_str::<ConstraintSource>(&text).unwrap(),
                source
            );
        }
        assert!(serde_json::from_str::<Instant>("\"n_a.middle\"").is_err());
        assert!(serde_json::from_str::<ConstraintSource>("\"dependency:n_a:n_b\"").is_err());
    }
}
