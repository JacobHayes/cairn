//! Domain patches over HTTP, in process against the memory store (PRACTICES, Shell: API
//! tests): every rejection shape once, and the auth layer, request limits, and
//! observability every handler carries (A15, A17, H5).
#![cfg(test)]

mod in_process {
    use axum::body::Bytes;
    use axum::http::{HeaderValue, Method, StatusCode};
    use cairn_api::error::status_of;
    use cairn_api::wire::{Capabilities, PatchAnswer, Problem, ProblemCode};
    use std::collections::BTreeSet;

    use cairn_schema::{Rejection, UndecidedConsequence, ViolationCode};

    use crate::support::{self, World, get, ok, post, request};

    const JOURNEY: &str = "/api/journeys/j_vendor_eval";

    fn violation_codes(rejection: &Rejection) -> Vec<ViolationCode> {
        let Rejection::Invalid { violations } = rejection else {
            panic!("an invalid rejection, not {rejection:#?}")
        };
        let mut codes: Vec<_> = violations
            .as_slice()
            .iter()
            .map(|found| found.code)
            .collect();
        codes.sort();
        codes
    }

    /// A17, H5: a journey is created at revision 0 and patched; the patch resubmitted after
    /// its response was lost is answered from its receipt; its id with other content is
    /// refused.
    #[tokio::test]
    async fn a_patch_lands_once_and_its_resubmission_is_answered_from_its_receipt() {
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        let answer = |reply| ok::<PatchAnswer>(&reply);
        let first = support::patch(
            "p_note",
            "{journey: j_vendor_eval}",
            1,
            "- op: add_annotation\n  annotation: {key: a_note, note: Called the vendor.}\n",
        );
        let landed = answer(post(&ann, &format!("{JOURNEY}/patches"), &request(&first)).await);
        let PatchAnswer::Applied { receipt, .. } = &landed else {
            panic!("applied, not {landed:#?}")
        };
        assert_eq!(receipt.revision.get(), 2);
        let again = answer(post(&ann, &format!("{JOURNEY}/patches"), &request(&first)).await);
        assert_eq!(
            again,
            PatchAnswer::AlreadyApplied {
                receipt: receipt.clone()
            }
        );

        let mut reused = first.clone();
        reused.base_revision = receipt.revision;
        let answered = post(&ann, &format!("{JOURNEY}/patches"), &request(&reused)).await;
        assert_eq!(answered.status, StatusCode::CONFLICT);
        let rejection: Rejection = answered.json().unwrap();
        assert!(
            matches!(rejection, Rejection::PatchIdReused { .. }),
            "{rejection:#?}"
        );
        let revision = world.revision("j_vendor_eval").await;
        assert_eq!(
            revision,
            receipt.revision.get(),
            "nothing was applied twice"
        );
    }

    /// D4, D7: completing work whose relevance waits on an unanswered decision is applied
    /// and answered with the warning naming that decision.
    #[tokio::test]
    async fn finishing_undecided_work_is_applied_with_a_warning() {
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        let complete = support::patch(
            "p_baseline",
            "{journey: j_vendor_eval}",
            1,
            "- op: transition\n  node: n_baseline\n  transition: complete\n",
        );
        let landed = ok::<PatchAnswer>(
            &post(&ann, &format!("{JOURNEY}/patches"), &request(&complete)).await,
        );
        let PatchAnswer::Applied { consequences, .. } = &landed else {
            panic!("applied, not {landed:#?}")
        };
        let caused = &consequences[&"j_vendor_eval".parse().unwrap()];
        assert_eq!(
            caused.undecided,
            [UndecidedConsequence {
                node: "n_baseline".parse().unwrap(),
                unanswered: BTreeSet::from(["n_comparison_set".parse().unwrap()]),
            }]
        );
    }

    /// A15: a patch with three independent violations is answered 422 with all three, by
    /// path, exactly as the engine reports them.
    #[tokio::test]
    async fn three_violations_arrive_as_three() {
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let invalid = support::patch(
            "p_three",
            "{journey: j_vendor_eval}",
            2,
            "- op: transition\n  node: n_kickoff\n  transition: reach\n\
             - op: transition\n  node: n_purpose\n  transition: start\n\
             - op: transition\n  node: n_nowhere\n  transition: start\n\
             - op: answer\n  decision: n_partner_runs\n  value: {text: maybe}\n",
        );
        let reply = post(&ann, &format!("{JOURNEY}/patches"), &request(&invalid)).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
        let rejection: Rejection = reply.json().unwrap();
        let mut expected = vec![
            ViolationCode::IllegalTransition,
            ViolationCode::UnresolvedReference,
            ViolationCode::AnswerTypeMismatch,
        ];
        expected.sort();
        assert_eq!(violation_codes(&rejection), expected);
        assert_eq!(
            world.revision("j_vendor_eval").await,
            2,
            "nothing was applied"
        );
    }

    /// H5: a patch drafted before another landed is answered 409 with the revision it named,
    /// the current one, and what the intervening events touched.
    #[tokio::test]
    async fn a_stale_patch_is_answered_with_what_intervened() {
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let bob = world.signed_in("bob");
        let note = |id: &str, key: &str| {
            let mutations =
                format!("- op: add_annotation\n  annotation: {{key: {key}, note: Seen.}}\n");
            support::patch(id, "{journey: j_vendor_eval}", 2, &mutations)
        };
        ok::<PatchAnswer>(
            &post(
                &bob,
                &format!("{JOURNEY}/patches"),
                &request(&note("p_bob", "a_bob")),
            )
            .await,
        );
        let reply = post(
            &ann,
            &format!("{JOURNEY}/patches"),
            &request(&note("p_ann", "a_ann")),
        )
        .await;
        assert_eq!(reply.status, StatusCode::CONFLICT);
        let Rejection::Stale {
            conflicts,
            intervening,
        } = reply.json().unwrap()
        else {
            panic!("stale")
        };
        assert_eq!(
            (conflicts[0].expected.get(), conflicts[0].current.get()),
            (2, 3)
        );
        assert!(!intervening.is_empty());
    }

    /// What a malformed request sends.
    enum Sent {
        Nothing,
        Json(&'static str),
        Text(&'static str),
        ElsewhereJourney,
    }

    /// One malformed request and the problem it is answered with, at that problem's
    /// status.
    struct Malformed {
        method: Method,
        target: &'static str,
        sent: Sent,
        problem: ProblemCode,
    }

    fn malformed_requests() -> Vec<Malformed> {
        use ProblemCode as P;
        let case = |method: Method, target, sent, problem| Malformed {
            method,
            target,
            sent,
            problem,
        };
        let (get, post) = (Method::GET, Method::POST);
        let patches = "/api/journeys/j_vendor_eval/patches";
        vec![
            case(
                post.clone(),
                patches,
                Sent::Text("{}"),
                P::UnsupportedMediaType,
            ),
            case(
                post.clone(),
                patches,
                Sent::Json(r#"{"patch": {"id": 7}}"#),
                P::BadRequest,
            ),
            case(
                post.clone(),
                patches,
                Sent::ElsewhereJourney,
                P::TargetMismatch,
            ),
            case(
                post,
                "/api/journeys/%FF/patches",
                Sent::Json("{}"),
                P::BadRequest,
            ),
            case(get.clone(), "/nowhere", Sent::Nothing, P::NoSuchEndpoint),
            case(
                get,
                "/api/deployment/patches",
                Sent::Nothing,
                P::MethodNotAllowed,
            ),
        ]
    }

    /// Every malformed request is answered with its problem, and the problem with the
    /// request id the response header names.
    #[tokio::test]
    async fn a_malformed_request_is_answered_with_its_problem() {
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        let elsewhere = request(&support::patch(
            "p_elsewhere",
            "{journey: j_elsewhere}",
            0,
            "- op: create_journey\n  name: Elsewhere\n",
        ));
        let json = HeaderValue::from_static("application/json");
        for case in malformed_requests() {
            let (content_type, body) = match case.sent {
                Sent::Nothing => (None, Bytes::new()),
                Sent::Json(text) => (Some(json.clone()), Bytes::from_static(text.as_bytes())),
                Sent::Text(text) => (None, Bytes::from_static(text.as_bytes())),
                Sent::ElsewhereJourney => (Some(json.clone()), Bytes::from(elsewhere.to_string())),
            };
            let (method, target) = (case.method, case.target);
            let reply = ann
                .send_raw(method.clone(), target, content_type, body)
                .await
                .unwrap();
            assert_eq!(reply.status, status_of(case.problem), "{method} {target}");
            let problem: Problem = reply.json().unwrap();
            assert_eq!(problem.error, case.problem, "{method} {target}");
            let header = reply.headers.get("x-request-id").unwrap().to_str().unwrap();
            assert_eq!(problem.request_id.as_deref(), Some(header));
        }
    }

    /// The auth layer stands in front of every endpoint but the public health check, and the
    /// request body limit before any parse. The root assembles the assistant, so its
    /// endpoints are served too.
    #[tokio::test]
    async fn requests_without_credentials_or_over_the_body_limit_are_refused() {
        let provider = std::sync::Arc::new(cairn_assistant::scripted::ScriptedProvider::default());
        let world = World::start_with_assistant(provider).await;
        let anonymous = world.anonymous();
        for endpoint in cairn_api::endpoints::ALL {
            let placeholders = endpoint.path.matches('{').count();
            let target = endpoint.path_with(&vec!["x_1"; placeholders]);
            let reply = anonymous
                .send(endpoint.method.clone(), &target, None)
                .await
                .unwrap();
            let expected = if endpoint.public {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            };
            assert_eq!(reply.status, expected, "{target}");
        }
        let ann = world.signed_in("ann");
        let limit = usize::try_from(cairn_api::limits::REQUEST_BYTES_MAX).unwrap();
        let mut body = vec![b' '; limit + 1];
        body[0] = b'{';
        let reply = ann
            .send_raw(
                Method::POST,
                "/api/deployment/patches",
                Some(HeaderValue::from_static("application/json")),
                Bytes::from(body),
            )
            .await
            .unwrap();
        assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            reply.json::<Problem>().unwrap().error,
            ProblemCode::PayloadTooLarge
        );
    }

    /// Observable: each request is counted and timed by endpoint, and each patch by
    /// outcome.
    #[tokio::test]
    async fn requests_and_patches_are_counted() {
        let recorder = metrics_util::debugging::DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let _installed = metrics::set_default_local_recorder(&recorder);
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        get::<Capabilities>(&ann, "/api/capabilities").await;
        let counted: Vec<(String, Vec<(String, String)>)> = snapshotter
            .snapshot()
            .into_vec()
            .into_iter()
            .map(|(key, _, _, _)| {
                let labels = key
                    .key()
                    .labels()
                    .map(|label| (label.key().to_owned(), label.value().to_owned()));
                (key.key().name().to_owned(), labels.collect())
            })
            .collect();
        let has = |name: &str, label: (&str, &str)| {
            counted.iter().any(|(found, labels)| {
                found == name
                    && labels
                        .iter()
                        .any(|(key, value)| (key.as_str(), value.as_str()) == label)
            })
        };
        assert!(has(
            cairn_api::observe::REQUESTS,
            ("endpoint", "/api/capabilities")
        ));
        assert!(has(
            cairn_api::observe::REQUEST_DURATION,
            ("endpoint", "/api/journeys/{id}/patches")
        ));
        assert!(has(cairn_api::observe::PATCHES, ("outcome", "applied")));
    }
}
