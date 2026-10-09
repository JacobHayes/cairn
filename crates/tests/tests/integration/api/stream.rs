//! The SSE revision stream over HTTP, in process against the memory store (H6): current
//! revisions first, a tick for each commit, and the subscriber limit.
#![cfg(test)]

mod in_process {
    use axum::http::{Method, StatusCode};
    use cairn_api::client::{EventStream, Opened, SseEvent, Transport};
    use cairn_api::limits::SSE_SUBSCRIBER_COUNT_MAX;
    use cairn_api::stream::TICK_EVENT;
    use cairn_api::wire::{Problem, ProblemCode, Tick};
    use cairn_schema::{Domain, RevisionOf};

    use crate::api::support::{self, World, post, request};

    async fn open(transport: &Transport, target: &str) -> EventStream {
        match transport.stream(target).await.unwrap() {
            Opened::Stream(stream) => *stream,
            Opened::Refused(reply) => panic!("refused: {reply:?}"),
        }
    }

    async fn next_tick(stream: &mut EventStream) -> Tick {
        let event: SseEvent = stream.next_event().await.unwrap().unwrap();
        assert_eq!(event.event.as_deref(), Some(TICK_EVENT));
        serde_json::from_str(&event.data).unwrap()
    }

    fn of(domain: Domain) -> RevisionOf {
        RevisionOf::Domain(domain)
    }

    /// H6: a subscriber is told the current revision of everything it watches at once,
    /// then a tick for each commit; one watching a journey that does not exist hears
    /// revision 0.
    #[tokio::test]
    async fn a_stream_starts_from_current_revisions_then_ticks() {
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let journey = Domain::Journey("j_vendor_eval".parse().unwrap());
        let target = "/api/events/stream?domain=journey:j_vendor_eval&domain=deployment&domain=journey:j_none";
        let mut stream = open(&ann, target).await;
        let mut first = vec![
            next_tick(&mut stream).await,
            next_tick(&mut stream).await,
            next_tick(&mut stream).await,
        ];
        first.sort();
        let current = |domain: Domain, number| Tick {
            of: of(domain),
            revision: number,
        };
        let revision = |number: u32| number.try_into().unwrap();
        let missing = Domain::Journey("j_none".parse().unwrap());
        let mut expected = vec![
            current(journey.clone(), revision(2)),
            current(Domain::Deployment, revision(1)),
            current(missing, revision(0)),
        ];
        expected.sort();
        assert_eq!(first, expected);

        let note = support::patch(
            "p_seen",
            "{journey: j_vendor_eval}",
            2,
            "- op: add_annotation\n  annotation: {key: a_seen, note: Seen.}\n",
        );
        let bob = world.signed_in("bob");
        support::ok::<serde_json::Value>(
            &post(&bob, "/api/journeys/j_vendor_eval/patches", &request(&note)).await,
        );
        assert_eq!(next_tick(&mut stream).await, current(journey, revision(3)));
    }

    /// The process holds at most `SSE_SUBSCRIBER_COUNT_MAX` subscribers; past that a stream
    /// is refused with 503 and `Retry-After`, until one goes away.
    #[tokio::test]
    async fn past_the_subscriber_limit_a_stream_is_refused_until_one_goes_away() {
        let world = World::start().await;
        let ann = world.signed_in("ann");
        let target = "/api/events/stream?domain=deployment";
        let mut held = Vec::new();
        for _ in 0..SSE_SUBSCRIBER_COUNT_MAX {
            held.push(open(&ann, target).await);
        }
        let Opened::Refused(reply) = ann.stream(target).await.unwrap() else {
            panic!("refused past the limit")
        };
        assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(reply.headers.contains_key("retry-after"));
        assert_eq!(
            reply.json::<Problem>().unwrap().error,
            ProblemCode::SubscriberLimit
        );

        drop(held.pop());
        while world.notifier.subscriber_count() >= SSE_SUBSCRIBER_COUNT_MAX as usize {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        let mut again = open(&ann, target).await;
        assert_eq!(next_tick(&mut again).await.of, of(Domain::Deployment));
    }

    /// What a stream watches must be named, and named rightly.
    #[tokio::test]
    async fn a_stream_must_name_what_it_watches() {
        let world = World::start().await;
        let ann = world.signed_in("ann");
        for target in [
            "/api/events/stream",
            "/api/events/stream?domain=node:n_a",
            "/api/events/stream?watch=journeys",
        ] {
            let reply = ann.send(Method::GET, target, None).await.unwrap();
            assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{target}");
        }
    }
}
