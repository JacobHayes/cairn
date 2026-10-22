//! The caller and their agent tokens (H2, H3).

use std::collections::BTreeSet;

use cairn_schema::{AgentId, Date, Email, IdentityKey, JourneyId, Slug, Timestamp, Title, UserId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// PRD: H2, H3.
/// The caller and the identities their verified emails name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Caller {
    /// The user.
    pub user: UserId,
    /// The agent acting for them, when the request carries an agent token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentId>,
    /// Their identities, which "mine" covers together.
    pub identities: BTreeSet<IdentityKey>,
    /// The logins they sign in with, by provider and subject.
    pub logins: Vec<LinkedLogin>,
}

impl Caller {
    /// The caller as the service knows them, acting through `agent` when a token carried the
    /// request.
    #[must_use]
    pub fn of(caller: crate::Caller, agent: Option<AgentId>) -> Self {
        Self {
            user: caller.user,
            agent,
            identities: caller.identities,
            logins: caller.logins.into_iter().map(Into::into).collect(),
        }
    }
}

// ARCHITECTURE: Auth: users and logins.
/// One login a user signs in with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkedLogin {
    /// The provider, by its configured name.
    pub provider: Slug,
    /// The provider's subject for the account.
    pub subject: Title,
    /// The name the provider gave the account at the last sign-in, if it gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Title>,
    // PRD: H3.
    /// The emails the provider marked verified at the last sign-in; only these match an
    /// identity.
    pub verified_emails: BTreeSet<Email>,
    /// When it was linked.
    pub linked_at: Timestamp,
}

impl From<cairn_store::LoginRecord> for LinkedLogin {
    fn from(record: cairn_store::LoginRecord) -> Self {
        Self {
            provider: record.provider,
            subject: record.subject,
            name: record.name,
            verified_emails: record.verified_emails,
            linked_at: record.linked_at,
        }
    }
}

// PRD: H3.
/// A user and the name people see for them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UserName {
    /// The user.
    pub user: UserId,
    /// Their name: their identity's name, else its first email; with no identity, the name
    /// their login gave, else its first verified email.
    pub name: Title,
}

impl From<crate::UserName> for UserName {
    fn from(name: crate::UserName) -> Self {
        Self {
            user: name.user,
            name: name.name,
        }
    }
}

// PRD: C20.
/// Where a person is in the tour of Cairn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TourStanding {
    /// No record: they have not started or dismissed it.
    None,
    /// A run is under way, with a practice journey.
    Started,
    /// They went through it to the end.
    Finished,
    /// They turned it down.
    Dismissed,
}

impl From<Option<cairn_store::TourState>> for TourStanding {
    fn from(state: Option<cairn_store::TourState>) -> Self {
        match state {
            None => TourStanding::None,
            Some(cairn_store::TourState::Started) => TourStanding::Started,
            Some(cairn_store::TourState::Finished) => TourStanding::Finished,
            Some(cairn_store::TourState::Dismissed) => TourStanding::Dismissed,
        }
    }
}

impl TourStanding {
    /// The stored state, none for no record.
    #[must_use]
    pub fn state(self) -> Option<cairn_store::TourState> {
        match self {
            TourStanding::None => None,
            TourStanding::Started => Some(cairn_store::TourState::Started),
            TourStanding::Finished => Some(cairn_store::TourState::Finished),
            TourStanding::Dismissed => Some(cairn_store::TourState::Dismissed),
        }
    }
}

/// The caller's place in the tour of Cairn, and today in the deployment's zone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tour {
    /// Where they are.
    pub state: TourStanding,
    /// The practice journey of a started run; it may have been deleted since.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub practice_journey: Option<JourneyId>,
    /// Today in the deployment's zone: the date the practice journey's screens call today.
    pub today: Date,
}

impl From<crate::TourView> for Tour {
    fn from(view: crate::TourView) -> Self {
        Self {
            state: view.record.as_ref().map(|record| record.state).into(),
            practice_journey: view.record.and_then(|record| record.practice_journey),
            today: view.today,
        }
    }
}

/// The record a tour write expects to replace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TourExpect {
    /// Where the writer last saw them.
    pub state: TourStanding,
    /// The practice journey of a started record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub practice_journey: Option<JourneyId>,
}

/// `PUT /api/users/me/tour`: where the caller's tour goes, and the record it replaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TourWrite {
    /// The new place: `started` carries the practice journey; `finished` and `dismissed`
    /// carry none.
    pub state: TourWriteState,
    /// The practice journey of a started run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub practice_journey: Option<JourneyId>,
    /// The record this replaces; a mismatch answers 409 with the current one.
    pub expect: TourExpect,
}

/// Where a tour write puts the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TourWriteState {
    /// A run begins.
    Started,
    /// A run ends, gone through.
    Finished,
    /// A run ends, turned down.
    Dismissed,
}

impl From<TourWriteState> for cairn_store::TourState {
    fn from(state: TourWriteState) -> Self {
        match state {
            TourWriteState::Started => cairn_store::TourState::Started,
            TourWriteState::Finished => cairn_store::TourState::Finished,
            TourWriteState::Dismissed => cairn_store::TourState::Dismissed,
        }
    }
}
