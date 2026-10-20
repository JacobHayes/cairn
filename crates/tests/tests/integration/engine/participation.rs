//! Pass 3, participation (E1, E2, E3, E5, B10): each step of E2's resolution order, explicit
//! empty stopping inheritance, roles filled through decisions while in effect, live
//! inheritance under role changes, `unassigned`, and the membership-loss flag.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::Records;
use cairn_schema::{IdentityKey, KindKey, ParticipationOrigin};
use support::key;

const VENDOR: &str = "j_vendor_eval";

fn kind(text: &str) -> KindKey {
    text.parse().unwrap()
}

fn identities(names: &[&str]) -> BTreeSet<IdentityKey> {
    names.iter().map(|name| name.parse().unwrap()).collect()
}

fn role(text: &str) -> cairn_schema::RoleKey {
    text.parse().unwrap()
}

/// Asserts a node's origin and identities for a kind.
fn resolves(
    records: &Records,
    journey: &str,
    cases: &[(&str, &str, Option<ParticipationOrigin>, &[&str])],
) {
    let derived = support::derived(records, journey);
    let participation = derived.participation();
    for (node, of, origin, expected) in cases {
        assert_eq!(
            participation.origin(&key(node), &kind(of)),
            origin.as_ref(),
            "{node} {of}"
        );
        assert_eq!(
            participation.identities(&key(node), &kind(of)),
            &identities(expected),
            "{node} {of}"
        );
    }
}

/// E2's order in the vendor evaluation once its up-front decisions fill the roles: a role on
/// the node, the nearest declaring ancestor's value, the default owner, and none.
#[test]
fn each_resolution_step_in_order() {
    let records = support::vendor_after(2);
    resolves(
        &records,
        VENDOR,
        &[
            (
                "n_reporting",
                "k_informed",
                Some(ParticipationOrigin::Role(role("r_stakeholders"))),
                &["who_d4m6uk2eqf5rbhzs", "who_e7h2jv3nxc4yaw6t"],
            ),
            (
                "n_final_report",
                "k_informed",
                Some(ParticipationOrigin::Ancestor(key("n_reporting"))),
                &["who_d4m6uk2eqf5rbhzs", "who_e7h2jv3nxc4yaw6t"],
            ),
            (
                "n_access",
                "k_owner",
                Some(ParticipationOrigin::DefaultOwner(role("r_eval_owner"))),
                &["who_k3d7qm2xw5tn6pfa"],
            ),
            ("n_access", "k_informed", None, &[]),
            (
                "n_final_report",
                "k_reviewer",
                Some(ParticipationOrigin::Role(role("r_findings_reviewer"))),
                &[],
            ),
        ],
    );
    let explicit = "- op: set_participation\n  node: n_setup\n  kind: k_owner\n  source: [who_d4m6uk2eqf5rbhzs]\n";
    resolves(
        &support::accepted_on(&records, VENDOR, explicit),
        VENDOR,
        &[
            (
                "n_setup",
                "k_owner",
                Some(ParticipationOrigin::Explicit),
                &["who_d4m6uk2eqf5rbhzs"],
            ),
            (
                "n_plan_draft",
                "k_owner",
                Some(ParticipationOrigin::Ancestor(key("n_setup"))),
                &["who_d4m6uk2eqf5rbhzs"],
            ),
        ],
    );
}

/// E2: an explicit empty participation means none and stops inheritance; removing it
/// restores inheritance. E1: a node no one owns is unassigned.
#[test]
fn explicit_empty_stops_inheritance_until_removed() {
    let records = support::vendor_after(2);
    let empty = "- op: set_participation\n  node: n_setup\n  kind: k_owner\n  source: []\n";
    let emptied = support::accepted_on(&records, VENDOR, empty);
    let derived = support::derived(&emptied, VENDOR);
    let participation = derived.participation();
    for node in ["n_setup", "n_plan", "n_plan_draft"] {
        assert!(
            participation
                .identities(&key(node), &KindKey::owner())
                .is_empty(),
            "{node}"
        );
        assert!(participation.is_unassigned(&key(node)), "{node}");
    }
    assert!(!participation.is_unassigned(&key("n_testing")));
    let cleared = "- op: clear_participation\n  node: n_setup\n  kind: k_owner\n";
    let restored = support::derived(&support::accepted_on(&emptied, VENDOR, cleared), VENDOR);
    assert_eq!(
        restored
            .participation()
            .identities(&key("n_plan_draft"), &KindKey::owner()),
        &identities(&["who_k3d7qm2xw5tn6pfa"])
    );
}

/// E3: a role with a filling decision holds its answer only while the decision is decided
/// and relevant; before that, everything it owns is unassigned.
#[test]
fn roles_fill_through_their_decisions() {
    let created = support::derived(&support::vendor_after(1), VENDOR);
    assert!(created.participation().is_unassigned(&key("n_access")));
    assert!(
        created
            .participation()
            .members(&role("r_eval_owner"))
            .is_empty()
    );
    let reopened = support::accepted_on(
        &support::vendor_after(2),
        VENDOR,
        "- op: transition\n  node: n_who_owns\n  transition: reopen\n",
    );
    let derived = support::derived(&reopened, VENDOR);
    assert!(
        derived
            .participation()
            .members(&role("r_eval_owner"))
            .is_empty()
    );
    assert!(derived.participation().is_unassigned(&key("n_access")));
}

/// E5: changing a role's identities re-derives what inherits from it and keeps explicit
/// overrides. B10: a per-member child whose identity leaves the role is flagged.
#[test]
fn role_changes_rederive_inheritance_and_flag_lost_members() {
    let records = support::after("hiring-loop", 4);
    let interviews = key("n_interviews");
    let interviewer = kind("k_interviewer");
    let before = support::derived(&records, "j_hiring");
    let panel = before
        .participation()
        .identities(&interviews, &interviewer)
        .clone();
    assert_eq!(panel.len(), 3);
    for child in ["n_interview_one", "n_interview_two", "n_interview_three"] {
        assert!(
            before
                .participation()
                .membership_lost(&key(child))
                .is_empty(),
            "{child}"
        );
    }
    let smaller = "- op: answer\n  decision: n_choose_panel\n  value: {identity_list: [who_k3d7qm2xw5tn6pfa, who_c5y3tw7ldp2aoxgm]}\n";
    let changed = support::accepted_on(&records, "j_hiring", smaller);
    let after = support::derived(&changed, "j_hiring");
    let participation = after.participation();
    assert_eq!(
        participation.identities(&interviews, &interviewer),
        &identities(&["who_k3d7qm2xw5tn6pfa", "who_c5y3tw7ldp2aoxgm"]),
        "inherited participation follows the role (E5)"
    );
    assert_eq!(
        participation.identities(&key("n_interview_three"), &interviewer),
        &identities(&["who_f2q5nr7xbd3kwzh4"]),
        "the explicit participation stays (E5)"
    );
    assert_eq!(
        participation.membership_lost(&key("n_interview_three")),
        &BTreeSet::from([interviewer.clone()])
    );
    assert!(
        participation
            .membership_lost(&key("n_interview_one"))
            .is_empty()
    );
}

/// E5, B5: a participation set on a node stays through a later change to the role it could
/// have followed, and is marked as a local edit.
#[test]
fn an_explicit_participation_survives_a_role_change() {
    let records = support::vendor_after(2);
    let records = support::accepted_on(
        &records,
        VENDOR,
        "- op: set_participation\n  node: n_access\n  kind: k_owner\n  source: [who_d4m6uk2eqf5rbhzs]\n",
    );
    let records = support::accepted_on(
        &records,
        VENDOR,
        "- op: answer\n  decision: n_who_owns\n  value: {identity: who_e7h2jv3nxc4yaw6t}\n",
    );
    let graph = support::vendor_graph(&records);
    let owner = &graph
        .nodes
        .get(&key("n_access"))
        .unwrap()
        .participations
        .as_map()[&kind("k_owner")];
    assert!(
        matches!(owner, cairn_schema::ParticipationSource::Identities(found) if found.iter().any(|e| e.as_str() == "who_d4m6uk2eqf5rbhzs"))
    );
    assert!(
        graph.state.local_edits[&key("n_access")]
            .contains(&cairn_schema::LocalEdit::Participation(kind("k_owner")))
    );
}
