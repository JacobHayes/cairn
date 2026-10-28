//! The per-kind state machines (D1): which transitions each kind's table has, from which
//! states, to which. Answering a decision is its own mutation and its own row
//! ([`answered`]); everything else is a [`Transition`]. Guards are not here: the evidence
//! guards are checked on the graph a patch produces (the guards stage), and relevance joins
//! with derive. Marking a group done is a skip-like terminal move: like a skip it is never
//! guarded.

use cairn_schema::{NodeKind, State, Transition};

/// A transition by name, without its reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Move {
    /// `start`.
    Start,
    /// `stop`.
    Stop,
    /// `complete`.
    Complete,
    /// `skip` (a reason is required, and the mutation's type carries it).
    Skip,
    /// `mark_done` (a group, with the reason the mutation's type carries).
    MarkDone,
    /// `reopen`.
    Reopen,
    /// `answer` and `revise`.
    Answer,
}

impl Move {
    /// Every move.
    pub const ALL: [Move; 7] = [
        Move::Start,
        Move::Stop,
        Move::Complete,
        Move::Skip,
        Move::MarkDone,
        Move::Reopen,
        Move::Answer,
    ];

    /// The move a transition mutation makes.
    #[must_use]
    pub fn of(transition: &Transition) -> Move {
        match transition {
            Transition::Start => Move::Start,
            Transition::Stop => Move::Stop,
            Transition::Complete | Transition::CompleteAnyway { .. } => Move::Complete,
            Transition::Skip { .. } => Move::Skip,
            Transition::MarkDone { .. } => Move::MarkDone,
            Transition::Reopen => Move::Reopen,
        }
    }
}

/// D1: the rows of `kind`'s table for `step`, as (from, to) pairs.
#[must_use]
pub fn rows(kind: NodeKind, step: Move) -> &'static [(State, State)] {
    use State::{Active, Decided, Derived, Done, Open, Pending, Skipped, Todo};
    match kind {
        NodeKind::Deliverable | NodeKind::Action => match step {
            Move::Start => &[(Todo, Active)],
            Move::Stop => &[(Active, Todo)],
            Move::Complete => &[(Todo, Done), (Active, Done)],
            Move::Skip => &[(Todo, Skipped), (Active, Skipped)],
            Move::Reopen => &[(Done, Todo), (Skipped, Todo)],
            Move::MarkDone | Move::Answer => &[],
        },
        NodeKind::Decision => match step {
            Move::Answer => &[(Open, Decided), (Decided, Decided)],
            Move::Skip => &[(Open, Skipped)],
            Move::Reopen => &[(Decided, Open), (Skipped, Open)],
            Move::Start | Move::Stop | Move::Complete | Move::MarkDone => &[],
        },
        NodeKind::Milestone => match step {
            Move::Complete => &[(Pending, Done)],
            Move::Skip => &[(Pending, Skipped)],
            Move::Reopen => &[(Done, Pending), (Skipped, Pending)],
            Move::Start | Move::Stop | Move::MarkDone | Move::Answer => &[],
        },
        NodeKind::Group => match step {
            Move::Skip => &[(Derived, Skipped)],
            Move::MarkDone => &[(Derived, Done)],
            Move::Reopen => &[(Skipped, Derived), (Done, Derived)],
            Move::Start | Move::Stop | Move::Complete | Move::Answer => &[],
        },
    }
}

/// D1: the state `step` moves a node of `kind` in `from` to, or `None` when the table has no
/// such row.
///
/// # Panics
///
/// Never: every row stays inside its kind's machine, which the assertion keeps so.
#[must_use]
pub fn next(kind: NodeKind, from: State, step: Move) -> Option<State> {
    let to = rows(kind, step)
        .iter()
        .find(|(row_from, _)| *row_from == from)
        .map(|(_, to)| *to);
    // Every row stays inside the kind's own machine.
    assert!(to.is_none_or(|to| to.legal_for(kind) && from.legal_for(kind)));
    to
}

/// D1: answering a decision: `open` to `decided`, or a revision of `decided`.
#[must_use]
pub fn answered(from: State) -> Option<State> {
    next(NodeKind::Decision, from, Move::Answer)
}

/// D4: whether a move needs its node in scope: a start, a complete and a first answer. Only
/// a deliverable's or action's completion has guards a bypass can cover, the evidence ones.
#[must_use]
pub fn is_guarded(kind: NodeKind, from: State, step: Move) -> bool {
    let guarded = match step {
        Move::Start | Move::Complete => true,
        Move::Answer => from == State::Open,
        Move::Stop | Move::Skip | Move::MarkDone | Move::Reopen => false,
    };
    guarded && next(kind, from, step).is_some()
}
