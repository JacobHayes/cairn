//! The API is complete (I1) and its document true: every service operation has an endpoint
//! the router serves and the OpenAPI document describes, and what the server answers
//! validates against the document's schema for that operation and status.
#![cfg(test)]

mod in_process {
    use std::collections::BTreeSet;
    use std::path::Path;

    use axum::http::Method;
    use cairn_api::client::Reply;
    use cairn_api::endpoints::{self as at, Endpoint};
    use serde_json::{Value, json};

    use crate::support::{self, World, post, request};

    /// Each public operation of the service, and the endpoints that offer it. Adding an
    /// operation to the service fails the coverage test until it is listed here with its
    /// endpoint.
    fn offered_by() -> Vec<(&'static str, Vec<&'static Endpoint>)> {
        vec![
            ("health", vec![&at::HEALTH]),
            ("capabilities", vec![&at::CAPABILITIES]),
            (
                "patch",
                vec![&at::PATCH_JOURNEY, &at::PATCH_ROUTE, &at::PATCH_DEPLOYMENT],
            ),
            ("journey", vec![&at::JOURNEY]),
            ("document", vec![&at::DOCUMENT]),
            ("routes", vec![&at::ROUTES]),
            ("route", vec![&at::ROUTE]),
            ("route_version", vec![&at::ROUTE_VERSION]),
            ("route_detail", vec![&at::ROUTE_VERSIONS]),
            ("deployment", vec![&at::DEPLOYMENT]),
            ("journeys", vec![&at::JOURNEYS]),
            ("search", vec![&at::SEARCH]),
            ("events", vec![&at::EVENTS]),
            ("entity", vec![&at::ENTITY]),
            ("subscribe", vec![&at::STREAM]),
            ("viewer", vec![&at::VIEWER]),
            ("snapshot", vec![&at::SNAPSHOT]),
            ("level", vec![&at::LEVEL]),
            ("trace", vec![&at::TRACE]),
            ("derived", vec![&at::DERIVED]),
            ("decision_view", vec![&at::DECISIONS]),
            ("timeline", vec![&at::TIMELINE]),
            ("status_summary", vec![&at::SUMMARY]),
            ("next", vec![&at::NEXT]),
            ("list", vec![&at::NODES]),
            ("mine", vec![&at::MINE]),
            ("node_detail", vec![&at::NODE]),
            ("explanations", vec![&at::EXPLANATIONS]),
            ("history", vec![&at::HISTORY]),
            (
                "create_proposal",
                vec![
                    &at::PROPOSE_JOURNEY,
                    &at::PROPOSE_ROUTE,
                    &at::PROPOSE_DEPLOYMENT,
                ],
            ),
            ("proposal", vec![&at::PROPOSAL]),
            ("edit_proposal", vec![&at::EDIT_PROPOSAL]),
            ("preview_proposal", vec![&at::PREVIEW_PROPOSAL]),
            ("apply_proposal", vec![&at::APPLY_PROPOSAL]),
            ("discard_proposal", vec![&at::DISCARD_PROPOSAL]),
            ("refresh_proposal", vec![&at::REFRESH_PROPOSAL]),
            ("propose_upgrade", vec![&at::UPGRADE]),
            ("propose_save_as_route", vec![&at::SAVE_AS_ROUTE]),
            ("propose_relink", vec![&at::RELINK]),
            ("import_route", vec![&at::IMPORT_ROUTE]),
            ("export_route", vec![&at::EXPORT_ROUTE]),
        ]
    }

    /// Methods of the service that are not operations: assembling it, and the settings the
    /// host reads.
    const NOT_OPERATIONS: [&str; 2] = ["new", "settings"];

    /// The public methods of every `impl<S: Store> Service<S>` block in the service's
    /// sources.
    fn service_methods() -> BTreeSet<String> {
        let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../service/src");
        let mut methods = BTreeSet::new();
        for entry in std::fs::read_dir(sources).unwrap() {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            let mut inside = false;
            for line in text.lines() {
                if line.starts_with("impl<S: Store> Service<S>") {
                    inside = true;
                } else if line == "}" {
                    inside = false;
                } else if inside {
                    let declared = line.trim_start().strip_prefix("pub async fn ");
                    let declared = declared.or_else(|| line.trim_start().strip_prefix("pub fn "));
                    if let Some(rest) = declared {
                        let name = rest.split(['(', '<']).next().unwrap();
                        methods.insert(name.to_owned());
                    }
                }
            }
        }
        assert!(
            methods.contains("patch"),
            "the service's methods were found"
        );
        methods
    }

    /// I1: every service operation has an endpoint, every endpoint is in the OpenAPI
    /// document, and the router serves it with its method (behind the auth layer, which
    /// answers 401 only on a path the router matched, unless the endpoint is public).
    #[tokio::test]
    async fn every_service_operation_has_an_endpoint() {
        let listed: BTreeSet<String> = offered_by()
            .iter()
            .map(|(operation, _)| (*operation).to_owned())
            .chain(NOT_OPERATIONS.iter().map(|name| (*name).to_owned()))
            .collect();
        assert_eq!(service_methods(), listed);

        let documented = cairn_api::openapi::operation_ids();
        let world = World::start().await;
        let anonymous = world.anonymous();
        let signed_in = world.signed_in("ann");
        for (operation, endpoints) in offered_by() {
            assert!(!endpoints.is_empty(), "{operation}");
            for endpoint in endpoints {
                assert!(cairn_api::endpoints::ALL.contains(&endpoint), "{operation}");
                assert!(documented.contains(&endpoint.operation), "{operation}");
                let placeholders = endpoint.path.matches('{').count();
                let target = endpoint.path_with(&vec!["x_1"; placeholders]);
                let reply = anonymous.send(endpoint.method.clone(), &target, None).await;
                let refused = reply.unwrap().status.as_u16() == 401;
                assert_eq!(refused, !endpoint.public, "{operation}: {target}");
                // Signed in, the router answers the endpoint's own method with anything but
                // 405 (the auth layer answers 401 before the router's method check).
                let reply = signed_in.send(endpoint.method.clone(), &target, None).await;
                let status = reply.unwrap().status;
                assert_ne!(status.as_u16(), 405, "{operation}: {target}");
            }
        }
    }

    /// Checks `reply` against the document's schema for `endpoint` at its status.
    fn conforms(document: &Value, endpoint: &Endpoint, reply: &Reply) {
        let errors = violations(document, endpoint, reply);
        assert!(
            errors.is_empty(),
            "{} {}: {errors:#?}",
            endpoint.operation,
            reply.status
        );
    }

    /// Where `reply` departs from the document's schema for `endpoint` at its status.
    fn violations(document: &Value, endpoint: &Endpoint, reply: &Reply) -> Vec<String> {
        let method = endpoint.method.as_str().to_ascii_lowercase();
        let status = reply.status.as_str();
        let operation = &document["paths"][endpoint.path][&method];
        let schema = &operation["responses"][status]["content"]["application/json"]["schema"];
        assert!(
            schema.is_object(),
            "{} answered {status}, undocumented",
            endpoint.operation
        );
        let root = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "allOf": [schema],
            "components": document["components"],
        });
        let validator = jsonschema::draft202012::new(&root).unwrap();
        let body: Value = serde_json::from_slice(&reply.body).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&body)
            .map(|e| e.to_string())
            .collect();
        errors
    }

    /// Reads whose answers, accepted and refused, the conformance test checks.
    fn documented_reads() -> Vec<(&'static Endpoint, &'static str)> {
        vec![
            (&at::HEALTH, "/healthz"),
            (&at::CAPABILITIES, "/capabilities"),
            (&at::JOURNEYS, "/journeys"),
            (&at::JOURNEY, "/journeys/j_vendor_eval"),
            (&at::DOCUMENT, "/journeys/j_vendor_eval/document"),
            (&at::ROUTES, "/routes?size=1"),
            (&at::ROUTE, "/routes/vendor-evaluation"),
            (&at::ROUTE_VERSIONS, "/routes/vendor-evaluation/versions"),
            (&at::ROUTE_VERSION, "/routes/vendor-evaluation/versions/1"),
            (&at::DEPLOYMENT, "/deployment"),
            (&at::ENTITY, "/entities/e_lead"),
            (&at::SEARCH, "/search?text=vendor"),
            (&at::EVENTS, "/events?size=3"),
            (&at::VIEWER, "/users/me"),
            (&at::TOKENS, "/users/me/tokens"),
            (&at::SNAPSHOT, "/journeys/j_vendor_eval/snapshot?depth=1"),
            (
                &at::LEVEL,
                "/journeys/j_vendor_eval/level?kind=group&kind=milestone",
            ),
            (&at::TRACE, "/journeys/j_vendor_eval/trace/n_access"),
            (&at::DECISIONS, "/journeys/j_vendor_eval/decisions"),
            (&at::TIMELINE, "/journeys/j_vendor_eval/timeline"),
            (&at::SUMMARY, "/journeys/j_vendor_eval/summary"),
            (&at::NEXT, "/journeys/j_vendor_eval/next?for_viewer=true"),
            (&at::NODES, "/journeys/j_vendor_eval/nodes?flag=blocked"),
            (&at::MINE, "/journeys/j_vendor_eval/mine"),
            (&at::NODE, "/journeys/j_vendor_eval/nodes/n_kickoff"),
            (
                &at::EXPLANATIONS,
                "/journeys/j_vendor_eval/nodes/n_kickoff/explanations/gravity",
            ),
            (
                &at::HISTORY,
                "/journeys/j_vendor_eval/history?node=n_kickoff",
            ),
            (&at::NODE, "/journeys/j_vendor_eval/nodes/n_nowhere"),
            (&at::NEXT, "/journeys/j_vendor_eval/next?sort=sideways"),
            (&at::JOURNEY, "/journeys/j_missing"),
            (&at::JOURNEYS, "/journeys?colour=blue"),
        ]
    }

    /// What the server answers, accepted and refused, is what the document says it answers.
    #[tokio::test]
    async fn answers_validate_against_the_document() {
        let document = cairn_api::openapi::document();
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let get = |target: &'static str| ann.send(Method::GET, target, None);
        let reads = documented_reads();
        for (endpoint, target) in reads {
            conforms(&document, endpoint, &get(target).await.unwrap());
        }
        let patches = "/journeys/j_vendor_eval/patches";
        let note = support::patch(
            "p_note",
            "{journey: j_vendor_eval}",
            2,
            "- op: add_annotation\n  annotation: {key: a_note, note: Seen.}\n",
        );
        for _ in 0..2 {
            conforms(
                &document,
                &at::PATCH_JOURNEY,
                &post(&ann, patches, &request(&note)).await,
            );
        }
        let mut stale = note.clone();
        stale.id = "p_stale".parse().unwrap();
        conforms(
            &document,
            &at::PATCH_JOURNEY,
            &post(&ann, patches, &request(&stale)).await,
        );
        let invalid = support::patch(
            "p_invalid",
            "{journey: j_vendor_eval}",
            3,
            "- op: transition\n  node: n_nowhere\n  transition: start\n",
        );
        conforms(
            &document,
            &at::PATCH_JOURNEY,
            &post(&ann, patches, &request(&invalid)).await,
        );
        let named = json!({"name": "nightly"});
        conforms(
            &document,
            &at::MINT_TOKEN,
            &post(&ann, "/users/me/tokens", &named).await,
        );
    }

    /// The health check answers without a credential, serving and once the store has
    /// failed closed, as the document says, and the document asks no credential of it.
    #[tokio::test]
    async fn the_health_check_is_public_and_answers_what_the_document_says() {
        let document = cairn_api::openapi::document();
        let operation = &document["paths"][at::HEALTH.path]["get"];
        assert_eq!(operation["security"], json!([]));
        let world = World::start().await;
        let anonymous = world.anonymous();
        let serving = anonymous.send(Method::GET, "/healthz", None).await.unwrap();
        assert_eq!(serving.status.as_u16(), 200);
        assert_eq!(serving.json::<Value>().unwrap()["status"], json!("ok"));
        conforms(&document, &at::HEALTH, &serving);
        world.faults.fail_closed();
        let closed = anonymous.send(Method::GET, "/healthz", None).await.unwrap();
        assert_eq!(closed.status.as_u16(), 503);
        assert_eq!(
            closed.json::<Value>().unwrap()["status"],
            json!("store_failed_closed")
        );
        conforms(&document, &at::HEALTH, &closed);
    }

    /// Proposal answers, accepted and refused, are what the document says they are.
    #[tokio::test]
    async fn proposal_answers_validate_against_the_document() {
        let document = cairn_api::openapi::document();
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let draft = json!({
            "title": "Note",
            "destination_revision": 2,
            "mutations": [{"op": "add_annotation", "annotation": {"key": "a_note", "note": "Seen."}}],
        });
        let create = json!({"patch_id": "p_pr", "id": "pr_note", "draft": draft});
        let proposals = "/journeys/j_vendor_eval/proposals";
        let mut lost = create.clone();
        lost["patch_id"] = json!("p_pr_lost");
        let mut reused = create.clone();
        reused["draft"]["title"] = json!("Another");
        for body in [&create, &create, &lost, &reused] {
            conforms(
                &document,
                &at::PROPOSE_JOURNEY,
                &post(&ann, proposals, body).await,
            );
        }
        let edit = json!({"patch_id": "p_edit", "base_revision": 7, "draft": draft});
        let edited = ann
            .send(Method::PATCH, "/proposals/pr_note", Some(&edit))
            .await;
        conforms(&document, &at::EDIT_PROPOSAL, &edited.unwrap());
        let get = ann.send(Method::GET, "/proposals/pr_note", None).await;
        conforms(&document, &at::PROPOSAL, &get.unwrap());
        let missing = ann.send(Method::GET, "/proposals/pr_missing", None).await;
        conforms(&document, &at::PROPOSAL, &missing.unwrap());
        let preview = post(&ann, "/proposals/pr_note/preview", &json!({})).await;
        conforms(&document, &at::PREVIEW_PROPOSAL, &preview);
        let refresh = json!({"patch_id": "p_refresh", "base_revision": 1});
        let refreshed = post(&ann, "/proposals/pr_note/refresh", &refresh).await;
        conforms(&document, &at::REFRESH_PROPOSAL, &refreshed);
        for reviewed in [1, 2, 2] {
            let apply =
                json!({"patch_id": format!("p_apply_{reviewed}"), "reviewed_revision": reviewed});
            let applied = post(&ann, "/proposals/pr_note/apply", &apply).await;
            conforms(&document, &at::APPLY_PROPOSAL, &applied);
        }
        let discard = json!({"patch_id": "p_discard", "base_revision": 2});
        let discarded = post(&ann, "/proposals/pr_note/discard", &discard).await;
        conforms(&document, &at::DISCARD_PROPOSAL, &discarded);
    }

    /// Route files and drafted proposals, accepted and refused, answer what the document says.
    #[tokio::test]
    async fn bulk_answers_validate_against_the_document() {
        let document = cairn_api::openapi::document();
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let route = "/routes/vendor-evaluation";
        for target in ["/export?version=1", "/export", "/export?version=9"] {
            let reply = ann
                .send(Method::GET, &format!("{route}{target}"), None)
                .await;
            conforms(&document, &at::EXPORT_ROUTE, &reply.unwrap());
        }
        let exported = ann
            .send(Method::GET, &format!("{route}/export?version=1"), None)
            .await
            .unwrap();
        let file: Value = exported.json().unwrap();
        let import = json!({"patch_id": "p_import", "file": file});
        let mut reused = import.clone();
        reused["file"]["name"] = json!("Renamed");
        let mut mismatched = import.clone();
        mismatched["file"]["route"] = json!("another-route");
        for body in [&import, &import, &reused, &mismatched] {
            let reply = post(&ann, &format!("{route}/import"), body).await;
            conforms(&document, &at::IMPORT_ROUTE, &reply);
        }
        let free = support::patch(
            "p_free",
            "{journey: j_free}",
            0,
            "- op: create_journey\n  name: Free\n",
        );
        post(&ann, "/journeys/j_free/patches", &request(&free)).await;
        let upgrade = json!({"patch_id": "p_upgrade", "proposal": "pr_upgrade", "to": 1});
        for journey in ["j_vendor_eval", "j_free", "j_missing"] {
            let reply = post(&ann, &format!("/journeys/{journey}/upgrade"), &upgrade).await;
            conforms(&document, &at::UPGRADE, &reply);
        }
        let save =
            json!({"patch_id": "p_save", "proposal": "pr_save", "route": "saved", "name": "Saved"});
        let saved = post(&ann, "/journeys/j_vendor_eval/save-as-route", &save).await;
        conforms(&document, &at::SAVE_AS_ROUTE, &saved);
        let lineage = json!({"route": "vendor-evaluation", "version": 1});
        let relink = json!({"patch_id": "p_relink", "proposal": "pr_relink", "lineage": lineage});
        let relinked = post(&ann, "/journeys/j_vendor_eval/relink", &relink).await;
        conforms(&document, &at::RELINK, &relinked);
    }

    /// Proposals for a route and for the deployment, and a preview with consequences, answer
    /// what the document says.
    #[tokio::test]
    async fn proposals_of_every_domain_validate_against_the_document() {
        let document = cairn_api::openapi::document();
        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        for (endpoint, target, id, draft) in crate::support::proposals_of_every_domain(&ann).await {
            let create = json!({"patch_id": format!("p_{id}"), "id": id, "draft": draft});
            conforms(&document, endpoint, &post(&ann, &target, &create).await);
            let preview = post(&ann, &format!("/proposals/{id}/preview"), &json!({})).await;
            conforms(&document, &at::PREVIEW_PROPOSAL, &preview);
        }
    }
}
