//! The fixtures walked through the server's service natively. The service is the one the
//! server binary assembles (over the memory store, as the API's in-process tests run it),
//! seeded with the same fixtures as the in-browser root, read as the root's local user at a
//! fixed clock after every scenario's last step.

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::sync::Arc;

use cairn_engine::{ApplyInputs, Graph, Records, consequences, derive};
use cairn_schema::{
    Actor, Consequences, Cursor, Domain, DomainDocument, ExplainedField, JourneyId, KeyRefs,
    LevelDisplay, LevelQuery, ListQuery, NextQuery, Node, NodeKey, NodeKind, Patch, PatchId,
    ProposalDraft, RankConstants, RouteFile, SnapshotScope, State, Timestamp, VersionNumber,
    from_yaml, to_yaml,
};
use cairn_service::{Call, Capabilities, DeploymentSettings, DomainPatch, Parts, Service, Written};
use cairn_store::{InProcessNotifier, MemoryStore};
use jiff::tz::TimeZone;

use super::{Case, CaseCall, Group};
use crate::error::json;
use crate::fixtures::{FIXTURES, seed, sign_in_local};
use crate::{
    AppliedLocally, ApplyRequest, ExportRequest, ImportRequest, PreviewRequest, Projection,
    now_or_never,
};

/// The clock every read and write here is made at: after every scenario's last step.
pub const NOW: &str = "2026-10-12T12:00:00Z";

/// What the server answered, or a panic naming what failed: the walk is a test oracle.
fn served<T, E: Debug>(result: Result<T, E>, what: &str) -> T {
    result.unwrap_or_else(|error| panic!("the server's {what}: {error:?}"))
}

/// A value parsed from YAML the walk wrote.
fn parsed<T: serde::de::DeserializeOwned>(yaml: &str) -> T {
    from_yaml(yaml).unwrap_or_else(|error| panic!("{error}\n{yaml}"))
}

fn now() -> Timestamp {
    served(NOW.parse(), "clock")
}

/// The in-browser root's local user, who every read and write here is made as.
fn local() -> Actor {
    Actor {
        user: parsed("u_local"),
        agent: None,
    }
}

fn reader() -> Call {
    Call {
        actor: local(),
        now: now(),
    }
}

/// The server: the service over the memory store, in UTC like the in-browser root, seeded
/// with every fixture, with the root's local identity signed in, so every read here is
/// derived with that identity's entities (one lead per fixture) as the viewer (H3).
fn server() -> Service<MemoryStore> {
    let settings = DeploymentSettings::new(parsed("UTC"), TimeZone::UTC, RankConstants::default());
    let store = Arc::new(MemoryStore::new());
    let service = Service::new(Parts {
        store: Arc::clone(&store),
        notifier: Arc::new(InProcessNotifier::new()),
        settings,
        capabilities: Capabilities::server(Vec::new(), false),
    });
    served(now_or_never(seed(&service)), "seeding");
    let actor = served(now_or_never(sign_in_local(&*store, now())), "sign-in");
    assert_eq!(actor, local(), "the walk reads as the root's local user");
    service
}

/// The server's domain document of `id`.
fn document(service: &Service<MemoryStore>, id: &JourneyId) -> DomainDocument {
    served(now_or_never(service.document(&reader(), id)), "document")
        .unwrap_or_else(|| panic!("the server holds {id}"))
}

/// The server's answer to `request`, as the API's JSON.
fn server_projection(
    service: &Service<MemoryStore>,
    id: &JourneyId,
    request: &Projection,
) -> String {
    let call = reader();
    let read = |what: &str| format!("{what} of {id}");
    match request {
        Projection::Level(query) => {
            let level = served(
                now_or_never(service.level(&call, id, query)),
                &read("level"),
            );
            json(&level.value)
        }
        Projection::AnswerEffects { key } => {
            let detail = served(
                now_or_never(service.node_detail(&call, id, key)),
                &read("answer effects"),
            );
            json(&detail.value.answer_effects)
        }
        Projection::Trace { key } => {
            json(&served(now_or_never(service.trace(&call, id, key)), &read("trace")).value)
        }
        Projection::DecisionView => json(
            &served(
                now_or_never(service.decision_view(&call, id)),
                &read("decision view"),
            )
            .value,
        ),
        Projection::Timeline => {
            json(&served(now_or_never(service.timeline(&call, id)), &read("timeline")).value)
        }
        Projection::StatusSummary => json(
            &served(
                now_or_never(service.status_summary(&call, id)),
                &read("summary"),
            )
            .value,
        ),
        Projection::Next { query } => {
            json(&served(now_or_never(service.next(&call, id, query)), &read("next")).value)
        }
        Projection::List { query } => {
            json(&served(now_or_never(service.list(&call, id, query)), &read("list")).value)
        }
        Projection::Mine { kinds } => {
            json(&served(now_or_never(service.mine(&call, id, kinds)), &read("mine")).value)
        }
        Projection::Snapshot { scope } => json(
            &served(
                now_or_never(service.snapshot(&call, id, scope)),
                &read("snapshot"),
            )
            .value,
        ),
        Projection::Explanations { key, field, cursor } => json(
            &served(
                now_or_never(service.explanations(&call, id, key, *field, *cursor)),
                &read("explanations"),
            )
            .value,
        ),
    }
}

/// Every projection over the document: every level (the top and each container), every
/// node's trace and explanation lists, and each view and list at its default query.
fn projections(document: &DomainDocument) -> Vec<Projection> {
    let nodes: Vec<&Node<KeyRefs>> = document.journey.graph.nodes.values().collect();
    let shown: BTreeSet<NodeKind> = NodeKind::ALL.into_iter().collect();
    let mut requests = vec![Projection::level(shown.clone(), None)];
    requests.extend(
        nodes
            .iter()
            .filter(|node| node.kind() == NodeKind::Group)
            .map(|node| Projection::level(shown.clone(), Some(node.key.clone()))),
    );
    // Collapsed groups and each relevance class left out, one at a time, over the whole graph.
    let groups: BTreeSet<NodeKey> = nodes
        .iter()
        .filter(|node| node.kind() == NodeKind::Group)
        .map(|node| node.key.clone())
        .collect();
    requests.push(Projection::Level(LevelQuery {
        collapsed: groups,
        ..LevelQuery::of_kinds(shown.clone(), None)
    }));
    for left_out in LevelDisplay::ALL {
        requests.push(Projection::Level(LevelQuery {
            display: LevelDisplay::ALL
                .into_iter()
                .filter(|class| *class != left_out)
                .collect(),
            ..LevelQuery::of_kinds(shown.clone(), None)
        }));
    }
    requests.extend([
        Projection::DecisionView,
        Projection::Timeline,
        Projection::StatusSummary,
        Projection::Next {
            query: NextQuery::default(),
        },
        Projection::List {
            query: ListQuery::default(),
        },
        Projection::Mine {
            kinds: BTreeSet::new(),
        },
        Projection::Snapshot {
            scope: SnapshotScope::default(),
        },
    ]);
    for node in nodes {
        if node.kind() == NodeKind::Decision {
            requests.push(Projection::AnswerEffects {
                key: node.key.clone(),
            });
        }
        requests.push(Projection::Trace {
            key: node.key.clone(),
        });
        for field in [
            ExplainedField::Gravity,
            ExplainedField::Leverage,
            ExplainedField::StillWaiting,
        ] {
            requests.push(Projection::Explanations {
                key: node.key.clone(),
                field,
                cursor: Cursor::START,
            });
        }
    }
    requests
}

/// A projection request's name in a case: its tag and its arguments.
fn named(request: &Projection) -> String {
    format!("project {}", json(request))
}

/// The journey's short name, for the ids the walk mints.
fn short(id: &JourneyId) -> String {
    id.to_string().trim_start_matches("j_").to_owned()
}

/// Whether `mutations` applied to the document's journey cause a D7 consequence, worked out
/// with the engine directly (apply, then a derive on each side with no viewer), so the walk
/// picks its patch without the code it checks.
fn causes_consequences(document: &DomainDocument, mutations: &str) -> bool {
    let id = &document.journey.header.id;
    let patch: Patch = parsed(&format!(
        "id: p_wasm_try\ntarget:\n  journey: {id}\nbase_revision: {}\nmutations:\n{mutations}",
        document.journey.revision
    ));
    let mut records = Records {
        deployment: document.inputs.deployment.clone(),
        ..Records::default()
    };
    records
        .journeys
        .insert(id.clone(), document.journey.clone());
    let inputs = ApplyInputs {
        today: document.inputs.today,
        at: now(),
        actor: local(),
        note: None,
    };
    let Ok(applied) = cairn_engine::apply(&records, &patch, &inputs) else {
        return false;
    };
    let side = |journey: &cairn_schema::Journey, deployment: &cairn_schema::Deployment| {
        let graph = served(Graph::new(journey.graph.clone(), deployment), "graph");
        let mut derive_inputs = document.inputs.clone();
        derive_inputs.viewer = BTreeSet::new();
        derive_inputs.deployment = deployment.clone();
        let derived = derive(&graph, Some(journey.header.created_on), &derive_inputs);
        (graph, derived)
    };
    let after = applied.records();
    let (before_graph, before) = side(&document.journey, &document.inputs.deployment);
    let after_journey = served(after.journeys.get(id).ok_or("no journey"), "journey after");
    let (after_graph, after_derived) = side(after_journey, &after.deployment);
    consequences(&before_graph, &before, &after_graph, &after_derived) != Consequences::default()
}

/// The mutations of the walk's patch and second proposal: a note on the journey's first
/// node, starting the first action or deliverable to do on its next list when there is one,
/// and a pin before today on the first node it makes newly overdue (F2, D7), so the derive
/// changes and the patch causes consequences in every fixture.
///
/// # Panics
///
/// When no pin before today causes a consequence: the walk would check nothing of D7.
fn mutations(
    service: &Service<MemoryStore>,
    id: &JourneyId,
    document: &DomainDocument,
    note: &str,
) -> String {
    let nodes = document.journey.graph.nodes.as_map();
    let first = served(nodes.keys().next().ok_or("no node"), "first node");
    let next = served(
        now_or_never(service.next(&reader(), id, &NextQuery::default())),
        "next",
    );
    let mut yaml = format!(
        "- op: add_annotation\n  annotation:\n    key: a_wasm_{note}\n    node: {first}\n    note: Checked in the browser.\n"
    );
    // An action or deliverable to do starts (D1); a milestone is reached, a decision answered.
    let startable = next.value.items.iter().find(|row| {
        matches!(row.kind, NodeKind::Action | NodeKind::Deliverable) && row.state == State::Todo
    });
    if let Some(top) = startable {
        yaml += &[
            "- op: transition\n  node: ",
            top.key.as_str(),
            "\n  transition: start\n",
        ]
        .concat();
    }
    let pinned = nodes.keys().find_map(|key| {
        let candidate = format!("{yaml}- op: set_pin\n  node: {key}\n  date: \"2026-10-08\"\n");
        causes_consequences(document, &candidate).then_some(candidate)
    });
    pinned.unwrap_or_else(|| panic!("{id}: no pin before today causes a consequence"))
}

/// C14: two proposals created on the server against the journey, each previewed there and
/// asked of the browser host with the proposal as the server answers it.
fn previews(
    service: &Service<MemoryStore>,
    id: &JourneyId,
    document: &DomainDocument,
) -> Vec<Case> {
    let first = served(
        document
            .journey
            .graph
            .nodes
            .as_map()
            .keys()
            .next()
            .ok_or("no node"),
        "first node",
    );
    let drafts = [
        (
            "a note",
            format!(
                "- op: add_annotation\n  annotation:\n    key: a_wasm_proposed\n    node: {first}\n    note: Proposed in the browser.\n"
            ),
        ),
        (
            "a note and a start",
            mutations(service, id, document, "proposed_start"),
        ),
    ];
    let destination = Domain::Journey(id.clone());
    drafts
        .iter()
        .enumerate()
        .map(|(index, (label, mutations))| {
            let draft: ProposalDraft = parsed(&format!(
                "title: Wasm check {index}\ndestination_revision: {}\nmutations:\n{mutations}",
                document.journey.revision
            ));
            let proposal_id = parsed(&format!("pr_wasm_{}_{index}", short(id)));
            let patch_id = parsed(&format!("p_wasm_propose_{}_{index}", short(id)));
            served(
                now_or_never(service.create_proposal(
                    &reader(),
                    patch_id,
                    &destination,
                    &proposal_id,
                    draft,
                )),
                "proposal create",
            );
            let proposal = served(now_or_never(service.proposal(&proposal_id)), "proposal")
                .unwrap_or_else(|| panic!("{proposal_id}"));
            let review = served(
                now_or_never(service.preview_proposal(&reader(), &proposal_id)),
                "preview",
            );
            let request = PreviewRequest {
                proposal,
                versions: Vec::new(),
                segments: Vec::new(),
                at: now(),
                actor: local(),
            };
            Case {
                name: format!("preview {label}"),
                call: CaseCall::Preview {
                    request: Box::new(request),
                },
                expected: json(&review.preview),
            }
        })
        .collect()
}

/// Everything read over one journey's document: its derive, every projection, and two
/// previews.
fn journey_group(service: &Service<MemoryStore>, label: &str, id: &JourneyId) -> Group {
    let document = document(service, id);
    let derived = served(now_or_never(service.derived(&reader(), id)), "derive");
    let mut cases = vec![Case {
        name: "derive".to_owned(),
        call: CaseCall::Derive,
        expected: json(&derived.value),
    }];
    cases.extend(projections(&document).into_iter().map(|request| Case {
        name: named(&request),
        expected: server_projection(service, id, &request),
        call: CaseCall::Project { request },
    }));
    cases.extend(previews(service, id, &document));
    Group {
        label: label.to_owned(),
        document: Some(json(&document)),
        cases,
    }
}

/// A17, D7: the walk's patch, applied locally against the document and committed on the
/// server: the journey after and its consequences agree. Also its touched set (H5). Then the
/// journey after, derived and projected.
fn apply_and_after(service: &Service<MemoryStore>, group: &mut Group, id: &JourneyId) -> Group {
    let before = document(service, id);
    let patch: Patch = parsed(&format!(
        "id: p_wasm_apply_{}\ntarget:\n  journey: {id}\nbase_revision: {}\nmutations:\n{}",
        short(id),
        before.journey.revision,
        mutations(service, id, &before, "applied")
    ));
    let submitted = served(DomainPatch::new(patch.clone(), None), "domain patch");
    let written = served(now_or_never(service.patch(&reader(), &submitted)), "patch");
    let Written::Applied { consequences, .. } = written else {
        panic!("{id}: a new patch applies now")
    };
    let after = document(service, id);
    let applied = AppliedLocally {
        document: after.clone(),
        consequences: consequences.get(id).cloned().unwrap_or_default(),
    };
    group.cases.push(Case {
        name: "touched set of the patch".to_owned(),
        call: CaseCall::Touched {
            patch: Box::new(patch.clone()),
        },
        expected: json(&patch.touched()),
    });
    let request = ApplyRequest {
        patch,
        note: None,
        versions: Vec::new(),
        segments: Vec::new(),
        at: now(),
        actor: local(),
    };
    group.cases.push(Case {
        name: "apply the patch".to_owned(),
        call: CaseCall::Apply {
            request: Box::new(request),
        },
        expected: json(&applied),
    });
    after_group(
        service,
        &format!("{} after a patch", group.label),
        id,
        &after,
    )
}

/// The journey's derive and its main projections, over the server's document `after` a
/// change.
fn after_group(
    service: &Service<MemoryStore>,
    label: &str,
    id: &JourneyId,
    after: &DomainDocument,
) -> Group {
    let derived = served(now_or_never(service.derived(&reader(), id)), "derive");
    let requests = [
        Projection::level(NodeKind::ALL.into_iter().collect(), None),
        Projection::Next {
            query: NextQuery::default(),
        },
        Projection::Mine {
            kinds: BTreeSet::new(),
        },
        Projection::List {
            query: ListQuery::default(),
        },
        Projection::StatusSummary,
        Projection::DecisionView,
    ];
    let mut cases = vec![Case {
        name: "derive".to_owned(),
        call: CaseCall::Derive,
        expected: json(&derived.value),
    }];
    cases.extend(requests.into_iter().map(|request| Case {
        name: named(&request),
        expected: server_projection(service, id, &request),
        call: CaseCall::Project { request },
    }));
    Group {
        label: label.to_owned(),
        document: Some(json(after)),
        cases,
    }
}

/// A13: each fixture's route exported at version 1, that file imported back as a new draft,
/// and the draft exported, against the server's export and the draft its import opened.
fn route_group(service: &Service<MemoryStore>) -> Group {
    let mut cases = Vec::new();
    for fixture in &FIXTURES {
        let Some(file) = fixture.route else { continue };
        let route_id = parsed::<RouteFile>(file).route;
        let first = VersionNumber::FIRST;
        let route = || {
            served(now_or_never(service.route(&route_id)), "route")
                .unwrap_or_else(|| panic!("{route_id}"))
        };
        let version = served(
            now_or_never(service.route_version(&route_id, first)),
            "version",
        );
        let exported = served(
            now_or_never(service.export_route(&route_id, Some(first))),
            "export",
        )
        .unwrap_or_else(|| panic!("{route_id}"));
        cases.push(Case {
            name: format!("export {route_id} version 1"),
            call: CaseCall::Export {
                request: Box::new(ExportRequest {
                    route: route(),
                    version: version.clone(),
                }),
            },
            expected: json(&exported),
        });
        let text = served(to_yaml(&exported), "file");
        let patch_id: PatchId =
            parsed(&format!("p_wasm_import_{}", fixture.name.replace('-', "_")));
        served(
            now_or_never(service.import_route(&reader(), patch_id.clone(), &exported, None)),
            "import",
        );
        let draft = served(route().draft.ok_or("no draft"), "draft");
        cases.push(Case {
            name: format!("import {route_id} version 1 back"),
            call: CaseCall::Import {
                request: Box::new(ImportRequest {
                    file: text,
                    base: version,
                    patch_id,
                }),
            },
            expected: json(&draft.graph),
        });
        let draft_file = served(
            now_or_never(service.export_route(&route_id, None)),
            "export",
        )
        .unwrap_or_else(|| panic!("{route_id}"));
        cases.push(Case {
            name: format!("export {route_id}'s draft"),
            call: CaseCall::Export {
                request: Box::new(ExportRequest {
                    route: route(),
                    version: None,
                }),
            },
            expected: json(&draft_file),
        });
    }
    Group {
        label: "route files".to_owned(),
        document: None,
        cases,
    }
}

/// E6: one of the vendor evaluation's stakeholders merged into a new entity, then the journey
/// derived again over the new deployment. The merge moves neither a journey revision nor the
/// viewer (the lead), so only the deployment revision tells a cached derive apart.
fn merge_group(service: &Service<MemoryStore>) -> Group {
    let id: JourneyId = parsed("j_vendor_eval");
    let deployment = served(now_or_never(service.deployment()), "deployment").revision;
    let journey = document(service, &id).journey.revision;
    let create: Patch = parsed(&format!(
        "id: p_wasm_entity\ntarget: deployment\nbase_revision: {deployment}\nmutations:\n- op: create_entity\n  entity: {{key: e_stakeholder_again, name: Stakeholder A}}\n"
    ));
    served(
        now_or_never(service.patch(&reader(), &served(DomainPatch::new(create, None), "patch"))),
        "entity create",
    );
    let merge: Patch = parsed(&format!(
        "id: p_wasm_merge\ntarget: deployment\nbase_revision: {}\nmutations:\n- op: merge_entities\n  survivor: e_stakeholder_again\n  merged: e_stakeholder_a\n  journeys: {{{id}: {journey}}}\n",
        deployment.next()
    ));
    served(
        now_or_never(service.patch(&reader(), &served(DomainPatch::new(merge, None), "patch"))),
        "entity merge",
    );
    let after = document(service, &id);
    assert_eq!(after.journey.revision, journey, "a merge moves no journey");
    after_group(
        service,
        "vendor-evaluation after an entity merge",
        &id,
        &after,
    )
}

/// Every group the server walk gives, in the order it walks them.
#[must_use]
pub fn server_groups() -> Vec<Group> {
    let service = server();
    let journeys: Vec<(&str, JourneyId)> = FIXTURES
        .iter()
        .map(|fixture| (fixture.name, served(fixture.scenario(), "scenario").journey))
        .collect();
    let mut groups: Vec<Group> = journeys
        .iter()
        .map(|(label, id)| journey_group(&service, label, id))
        .collect();
    let mut after = Vec::new();
    for (group, (_, id)) in groups.iter_mut().zip(&journeys) {
        after.push(apply_and_after(&service, group, id));
    }
    groups.extend(after);
    groups.push(route_group(&service));
    groups.push(merge_group(&service));
    groups
}
