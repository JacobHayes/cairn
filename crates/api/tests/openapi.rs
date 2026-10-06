//! The API is complete (I1) and its document true: every service operation has an endpoint
//! the router serves and the OpenAPI document describes, and what the server answers
//! validates against the document's schema for that operation and status.
#![cfg(test)]

mod support;

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
            ("capabilities", vec![&at::CAPABILITIES]),
            (
                "patch",
                vec![&at::PATCH_JOURNEY, &at::PATCH_ROUTE, &at::PATCH_DEPLOYMENT],
            ),
            ("journey", vec![&at::JOURNEY]),
            ("document", vec![&at::DOCUMENT]),
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
        ]
    }

    /// Methods of the service that are not operations: assembling it, and the settings the
    /// host reads.
    const NOT_OPERATIONS: [&str; 2] = ["new", "settings"];

    /// The service operations brief 4.8 added, whose endpoints brief 4.9 adds (DECISIONS.md:
    /// the service and API split at the engine's 2.4 boundary). 4.9 moves each into
    /// `offered_by` with its endpoint and empties this list.
    const AWAITING_ENDPOINTS: &[&str] = &[
        "snapshot",
        "level",
        "trace",
        "decision_view",
        "timeline",
        "status_summary",
        "next",
        "list",
        "mine",
        "node_detail",
        "explanations",
        "history",
        "create_proposal",
        "edit_proposal",
        "discard_proposal",
        "proposal",
        "preview_proposal",
        "apply_proposal",
        "refresh_proposal",
        "propose_upgrade",
        "propose_save_as_route",
        "propose_relink",
        "import_route",
        "export_route",
    ];

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
    /// document, and the router serves it (behind the auth layer, which answers 401 only on
    /// a path the router matched).
    #[tokio::test]
    async fn every_service_operation_has_an_endpoint() {
        let listed: BTreeSet<String> = offered_by()
            .iter()
            .map(|(operation, _)| (*operation).to_owned())
            .chain(NOT_OPERATIONS.iter().map(|name| (*name).to_owned()))
            .chain(AWAITING_ENDPOINTS.iter().map(|name| (*name).to_owned()))
            .collect();
        assert_eq!(service_methods(), listed);

        let documented = cairn_api::openapi::operation_ids();
        let world = World::start().await;
        let anonymous = world.anonymous();
        for (operation, endpoints) in offered_by() {
            assert!(!endpoints.is_empty(), "{operation}");
            for endpoint in endpoints {
                assert!(cairn_api::endpoints::ALL.contains(&endpoint), "{operation}");
                assert!(documented.contains(&endpoint.operation), "{operation}");
                let placeholders = endpoint.path.matches('{').count();
                let target = endpoint.path_with(&vec!["x_1"; placeholders]);
                let reply = anonymous.send(endpoint.method.clone(), &target, None).await;
                assert_eq!(reply.unwrap().status.as_u16(), 401, "{operation}: {target}");
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

    /// What the server answers, accepted and refused, is what the document says it answers.
    #[tokio::test]
    async fn answers_validate_against_the_document() {
        let document = cairn_api::openapi::document();
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let get = |target: &'static str| ann.send(Method::GET, target, None);
        let reads = [
            (&at::CAPABILITIES, "/capabilities"),
            (&at::JOURNEYS, "/journeys"),
            (&at::JOURNEY, "/journeys/j_vendor_eval"),
            (&at::DOCUMENT, "/journeys/j_vendor_eval/document"),
            (&at::ROUTE, "/routes/vendor-evaluation"),
            (&at::ROUTE_VERSIONS, "/routes/vendor-evaluation/versions"),
            (&at::ROUTE_VERSION, "/routes/vendor-evaluation/versions/1"),
            (&at::DEPLOYMENT, "/deployment"),
            (&at::ENTITY, "/entities/e_lead"),
            (&at::SEARCH, "/search?text=vendor"),
            (&at::EVENTS, "/events?size=3"),
            (&at::VIEWER, "/users/me"),
            (&at::TOKENS, "/users/me/tokens"),
            (&at::JOURNEY, "/journeys/j_missing"),
            (&at::JOURNEYS, "/journeys?colour=blue"),
        ];
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
}
