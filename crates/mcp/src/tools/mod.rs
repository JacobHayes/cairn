//! The tools (ARCHITECTURE, MCP endpoint), in groups, each a module with its table and its
//! dispatch. Each tool maps to one service operation; its arguments are shaped for an
//! agent's loop, its output is bounded, and a list that can grow is paged.

mod derived;
mod proposals;
mod reads;
mod writes;

use std::collections::BTreeMap;

use cairn_schema::{Consequences, Cursor, Date, JourneyId, PatchReceipt, Revision};
use cairn_service::{Projected, Written};
use schemars::JsonSchema;
use serde::Serialize;

use crate::limits::PAGE_ITEM_COUNT_MAX;
use crate::toolset::Spec;

/// The tool groups, each dispatched by its own module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Group {
    /// Derived reads (I3, C2, C8, C10): computed at the caller's today, as the viewer.
    Derive,
    /// Store-backed reads.
    Read,
    /// Direct writes: each call is one patch.
    Write,
    /// Proposals: drafted, edited, and applied by id (I6).
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

/// A derived read with what it was derived from (D3, H6): the journey revision a later write
/// names as its base, the deployment revision, and the today it was derived for.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub(crate) struct At<T> {
    /// The journey revision it was derived at: the base revision of a write that follows.
    revision: Revision,
    /// The deployment revision it was derived over.
    deployment_revision: Revision,
    /// The today it was derived for, in the deployment's time zone (A9).
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

/// A page of a list that can grow: at most `page_item_count_max` items (PRACTICES, Explicit
/// limits), the cursor of the next page, and how many there are in all.
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

/// What a direct write answers: its receipt and what it newly caused (D7).
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub(crate) struct WriteOutput {
    /// `applied` now, or `already_applied`: the patch id was committed before with the same
    /// content and is answered from its receipt (H5), with no consequences.
    status: WriteStatus,
    /// The receipt: the revision the patch produced is the base of the next write.
    receipt: PatchReceipt,
    /// What it caused in each journey it changed (D7): newly stale nodes, shortfalls,
    /// finished work that may not apply while a decision is unanswered (D4), relevance
    /// changes, and the frontier's movement.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    consequences: BTreeMap<JourneyId, Consequences>,
}

/// Whether a write was applied now or answered from its receipt.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum WriteStatus {
    Applied,
    AlreadyApplied,
}

impl From<Written> for WriteOutput {
    fn from(written: Written) -> Self {
        match written {
            Written::Applied {
                receipt,
                consequences,
            } => Self {
                status: WriteStatus::Applied,
                receipt,
                consequences,
            },
            Written::AlreadyApplied { receipt } => Self {
                status: WriteStatus::AlreadyApplied,
                receipt,
                consequences: BTreeMap::new(),
            },
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
