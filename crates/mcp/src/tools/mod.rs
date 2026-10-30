//! The tools (ARCHITECTURE, MCP endpoint), in groups, each a module with its table and its
//! dispatch. Each tool maps to one service operation; its arguments are shaped for an
//! agent's loop, its output is bounded, and a list that can grow is paged.

mod derived;
mod proposals;
mod reads;
mod writes;

use std::collections::BTreeMap;

use cairn_schema::{
    Consequences, Cursor, Date, Domain, JourneyId, Notice, Proposal, ProposalId, ProposalStatus,
    Revision,
};
use cairn_service::{Projected, ProposalWritten, Written};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::limits::PAGE_ITEM_COUNT_MAX;
use crate::toolset::Spec;

/// The tool groups, each dispatched by its own module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Group {
    // PRD: I3, C2, C8, C10.
    /// Derived reads: computed at the caller's today, as the viewer.
    Derive,
    /// Store-backed reads.
    Read,
    /// Direct writes: each call is one patch.
    Write,
    // PRD: I6.
    /// Proposals: drafted, edited, and applied by id.
    Propose,
}

/// Every tool's entry, by group.
fn groups() -> [(Group, &'static [Spec]); 4] {
    [
        (Group::Derive, derived::SPECS),
        (Group::Read, reads::SPECS),
        (Group::Write, writes::SPECS),
        (Group::Propose, proposals::SPECS),
    ]
}

/// Every tool's entry, in the order they are listed.
pub(crate) fn specs() -> impl Iterator<Item = &'static Spec> {
    groups().into_iter().flat_map(|(_, specs)| specs.iter())
}

/// The entry of the tool named `name`, if one is.
pub(crate) fn spec(name: &str) -> Option<&'static Spec> {
    specs().find(|spec| spec.name == name)
}

/// The group of the tool named `name`, if one is.
pub(crate) fn group_of(name: &str) -> Option<Group> {
    groups()
        .into_iter()
        .find(|(_, specs)| specs.iter().any(|spec| spec.name == name))
        .map(|(group, _)| group)
}

// PRD: D3, H6.
/// A derived read with what it was derived from: the journey revision a later write
/// names as its base, the deployment revision, and the today it was derived for.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub(crate) struct At<T> {
    /// The journey revision it was derived at: the base revision of a write that follows.
    revision: Revision,
    /// The deployment revision it was derived over.
    deployment_revision: Revision,
    // PRD: A9.
    /// The today it was derived for, in the deployment's time zone.
    today: Date,
    #[serde(flatten)]
    value: T,
}

impl<T> At<T> {
    /// The service's projection, its value shaped by `shape`.
    pub(crate) fn of<U>(projected: Projected<U>, shape: impl FnOnce(U) -> T) -> Self {
        let Projected {
            revision,
            deployment_revision,
            today,
            value,
        } = projected;
        Self {
            revision,
            deployment_revision,
            today,
            value: shape(value),
        }
    }
}

// PRACTICES: Explicit limits.
/// A page of a list that can grow: at most `page_item_count_max` items, the cursor of the next
/// page, and how many there are in all.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub(crate) struct Paged<T> {
    /// The items on this page.
    pub items: Vec<T>,
    /// Pass as `cursor` for the next page; absent on the last.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Cursor>,
    /// How many items there are across every page.
    pub total: u32,
}

impl<T> Default for Paged<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            next: None,
            total: 0,
        }
    }
}

impl<T> Paged<T> {
    /// The list holds nothing.
    pub(crate) fn is_empty(&self) -> bool {
        self.total == 0
    }
}

/// The page of `items` (all of a list, in its order) that starts at `cursor`.
pub(crate) fn page<T>(items: Vec<T>, cursor: Cursor) -> Paged<T> {
    let total = u32::try_from(items.len()).unwrap_or(u32::MAX);
    let start = usize::try_from(cursor.position()).unwrap_or(usize::MAX);
    let size = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap_or(usize::MAX);
    let items: Vec<T> = items.into_iter().skip(start).take(size).collect();
    let end = cursor
        .position()
        .saturating_add(u32::try_from(items.len()).unwrap_or(u32::MAX));
    assert!(items.len() <= size, "a page holds at most the page limit");
    Paged {
        items,
        next: (end < total).then(|| Cursor::at(end)),
        total,
    }
}

/// The start of a list no tool continues: at most `page_item_count_max` items and how many
/// there are in all. Where the rest is read is said beside each use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub(crate) struct Cut<T> {
    /// The first items.
    pub items: Vec<T>,
    /// How many items there are in all.
    pub total: u32,
}

impl<T> Cut<T> {
    /// The first page of `items`.
    pub(crate) fn of(items: Vec<T>) -> Self {
        let Paged { items, total, .. } = page(items, Cursor::START);
        Self { items, total }
    }
}

// PRD: D7.
/// What a write answers: where it landed and what it newly caused. The arguments it was sent,
/// its content hash and a proposal's content are not answered back.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WriteOutput {
    // PRD: H5.
    /// `applied` now, or `already_applied`: the patch id was committed before with the same
    /// content and is answered from its receipt, with no consequences. A write to a proposal
    /// is `saved` now, or `already_saved`, answered from its receipt (fetch the proposal for
    /// where it stands), or `existing`, a create of a proposal the caller already holds,
    /// answered with it as it stands.
    status: WriteStatus,
    /// An applied write: the journey's, route's or deployment's revision it produced, the
    /// base of your next write when the status is `applied`; after `already_applied`, read
    /// again first, others may have written since. A write to a proposal: the proposal's
    /// editing revision, which review (`get_proposal`) and the next change to it name.
    revision: Revision,
    // PRD: D7, D4.
    /// What it caused in each journey it changed: newly stale nodes, shortfalls,
    /// finished work that may not apply while a decision is unanswered, relevance
    /// changes, and the frontier's movement.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    consequences: BTreeMap<JourneyId, Consequences>,
    // PRD: A20.
    /// Advisory notices about the route graph an import or publish leaves: work with no
    /// chain to or from the final milestone, which neither its priority nor its dates feel.
    /// The write was accepted; add the missing edge or date rule, or leave it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notices: Vec<Notice>,
    // PRD: I6.
    /// A write to a proposal: its id; absent when the answer is only a receipt
    /// (`already_saved`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    proposal: Option<ProposalId>,
    /// A write to a proposal: where it would land (a receipt names the destination too).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    destination: Option<Domain>,
    /// A write to a proposal: the destination revision it was drafted against; the base of
    /// anything you add to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    destination_revision: Option<Revision>,
    /// A write to a proposal: whether it is still open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state: Option<ProposalStatus>,
}

/// How a write ended.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
enum WriteStatus {
    Applied,
    AlreadyApplied,
    Saved,
    AlreadySaved,
    Existing,
}

impl WriteOutput {
    /// Whether the write was answered from the receipt of an earlier one, which this call did
    /// not apply.
    #[must_use]
    pub fn is_already_applied(&self) -> bool {
        matches!(self.status, WriteStatus::AlreadyApplied)
    }

    /// Whether the write went to a proposal rather than a domain.
    #[must_use]
    pub fn is_proposal(&self) -> bool {
        self.status > WriteStatus::AlreadyApplied
    }

    /// The revision the write produced, and what it newly caused in each journey it changed.
    #[must_use]
    pub fn into_parts(self) -> (Revision, BTreeMap<JourneyId, Consequences>) {
        (self.revision, self.consequences)
    }

    /// A proposal write's destination, when the answer says (a receipt does not).
    #[must_use]
    pub fn destination(&self) -> Option<&Domain> {
        self.destination.as_ref()
    }

    /// Whether the answer says where the proposal stands (a receipt does not).
    #[must_use]
    pub fn state_known(&self) -> bool {
        self.state.is_some()
    }

    /// Whether the answer says the proposal is discarded.
    #[must_use]
    pub fn is_discarded(&self) -> bool {
        self.state == Some(ProposalStatus::Discarded)
    }

    fn held(status: WriteStatus, proposal: Proposal) -> Self {
        Self {
            status,
            revision: proposal.revision,
            consequences: BTreeMap::new(),
            notices: Vec::new(),
            destination_revision: Some(proposal.draft.destination_revision),
            state: Some(proposal.status),
            proposal: Some(proposal.id),
            destination: Some(proposal.destination),
        }
    }

    fn receipt(status: WriteStatus, revision: Revision) -> Self {
        Self {
            status,
            revision,
            consequences: BTreeMap::new(),
            notices: Vec::new(),
            proposal: None,
            destination: None,
            destination_revision: None,
            state: None,
        }
    }
}

impl From<Written> for WriteOutput {
    fn from(written: Written) -> Self {
        match written {
            Written::Applied {
                receipt,
                consequences,
                notices,
            } => Self {
                consequences,
                notices,
                ..Self::receipt(WriteStatus::Applied, receipt.revision)
            },
            Written::AlreadyApplied { receipt } => {
                Self::receipt(WriteStatus::AlreadyApplied, receipt.revision)
            }
        }
    }
}

impl From<ProposalWritten> for WriteOutput {
    fn from(written: ProposalWritten) -> Self {
        match written {
            ProposalWritten::Saved { proposal, .. } => Self::held(WriteStatus::Saved, proposal),
            ProposalWritten::AlreadySaved { receipt } => Self {
                destination: Some(receipt.domain.clone()),
                ..Self::receipt(WriteStatus::AlreadySaved, receipt.revision)
            },
            ProposalWritten::Existing { proposal } => Self::held(WriteStatus::Existing, proposal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_hold_at_most_the_limit_and_continue_where_they_stop() {
        let limit = PAGE_ITEM_COUNT_MAX;
        let cases = [
            (0, Cursor::START, 0, None),
            (3, Cursor::START, 3, None),
            (limit + 1, Cursor::START, limit, Some(limit)),
            (limit + 1, Cursor::at(limit), 1, None),
            (3, Cursor::at(7), 0, None),
        ];
        for (count, cursor, shown, next) in cases {
            let items: Vec<u32> = (0..count).collect();
            let paged = page(items, cursor);
            let first = paged.items.first().copied();
            assert_eq!(paged.items.len(), shown as usize, "{count} from {cursor:?}");
            assert_eq!(paged.next, next.map(Cursor::at), "{count} from {cursor:?}");
            assert_eq!(paged.total, count);
            if shown > 0 {
                assert_eq!(first, Some(cursor.position()));
            }
        }
    }

    #[test]
    fn every_tool_has_one_group() {
        let names: Vec<&str> = specs().map(|spec| spec.name).collect();
        let distinct: std::collections::BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(names.len(), distinct.len(), "tool names are unique");
        for name in names {
            assert!(group_of(name).is_some(), "{name}");
        }
        assert_eq!(group_of("no_such_tool"), None);
    }
}
