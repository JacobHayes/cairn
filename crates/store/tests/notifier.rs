//! The in-process notifier (H6; ARCHITECTURE, Concurrency and notification): initial
//! revisions at once, nothing missed between a fetch and a subscription, per-domain
//! coalescing within the interval, and the subscriber limit. The clock is an input, so each
//! interleaving is spelled out rather than swept (DECISIONS.md: notifier point tests).

#[cfg(test)]
mod notifier {
    use std::collections::{BTreeMap, BTreeSet};
    use std::future::Future;
    use std::pin::pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::task::{Context, Poll, Wake, Waker};
    use std::time::Duration;

    use cairn_schema::{Domain, Revision, RevisionOf};
    use cairn_store::limits::{SSE_COALESCING_INTERVAL, SSE_SUBSCRIBER_COUNT_MAX};
    use cairn_store::query::Revisions;
    use cairn_store::{InProcessNotifier, Notifier, Take, Tick, Watch};

    fn journey(id: &str) -> RevisionOf {
        RevisionOf::Domain(Domain::Journey(id.parse().unwrap()))
    }

    fn route(id: &str) -> RevisionOf {
        RevisionOf::Domain(Domain::Route(id.parse().unwrap()))
    }

    fn proposal(id: &str) -> RevisionOf {
        RevisionOf::Proposal(id.parse().unwrap())
    }

    fn deployment() -> RevisionOf {
        RevisionOf::Domain(Domain::Deployment)
    }

    fn revision(value: u32) -> Revision {
        (0..value).fold(Revision::NONE, |at, _| at.next())
    }

    fn tick(of: RevisionOf, at: u32) -> Tick {
        Tick {
            of,
            revision: revision(at),
        }
    }

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    fn watching(watches: &[Watch]) -> BTreeSet<Watch> {
        watches.iter().cloned().collect()
    }

    /// The store's current revisions: deployment 3, journeys `j_a` at 2 and `j_b` at 5, route
    /// `vendor` at 1, proposal `pr_x` on `j_a` at 4.
    fn current() -> Revisions {
        Revisions {
            deployment: revision(3),
            journeys: BTreeMap::from([
                ("j_a".parse().unwrap(), revision(2)),
                ("j_b".parse().unwrap(), revision(5)),
            ]),
            routes: BTreeMap::from([("vendor".parse().unwrap(), revision(1))]),
            proposals: BTreeMap::from([(
                "pr_x".parse().unwrap(),
                (Domain::Journey("j_a".parse().unwrap()), revision(4)),
            )]),
        }
    }

    fn ticks(take: Take) -> Vec<Tick> {
        match take {
            Take::Ticks(ticks) => ticks,
            other => panic!("expected ticks, got {other:?}"),
        }
    }

    /// The complete first take after seeding.
    fn current_of(take: Take) -> Vec<Tick> {
        match take {
            Take::Current(ticks) => ticks,
            other => panic!("expected the current revisions, got {other:?}"),
        }
    }

    #[test]
    fn a_new_subscriber_is_handed_the_current_revisions_of_what_it_watches_at_once() {
        let cases: [(&str, Vec<Watch>, Vec<Tick>); 4] = [
            (
                "every journey",
                vec![Watch::Journeys],
                vec![tick(journey("j_a"), 2), tick(journey("j_b"), 5)],
            ),
            (
                "every route and the deployment",
                vec![Watch::Routes, Watch::One(deployment())],
                vec![tick(route("vendor"), 1), tick(deployment(), 3)],
            ),
            (
                "one journey and its proposal",
                vec![Watch::One(journey("j_b")), Watch::One(proposal("pr_x"))],
                vec![tick(journey("j_b"), 5), tick(proposal("pr_x"), 4)],
            ),
            (
                "a journey the store does not list is at revision 0",
                vec![Watch::One(journey("j_new"))],
                vec![tick(journey("j_new"), 0)],
            ),
        ];
        for (case, watches, expected) in cases {
            let notifier = InProcessNotifier::new();
            let subscription = notifier.subscribe(watching(&watches)).unwrap();
            assert_eq!(subscription.take(ms(0)), Take::Empty, "{case}: unseeded");
            subscription.seed(&current());
            assert_eq!(current_of(subscription.take(ms(5))), expected, "{case}");
        }
    }

    #[test]
    fn a_commit_between_registering_and_seeding_is_never_missed() {
        // The subscription is registered, a commit moves j_a to 3 and is published, and only
        // then does the store's read of the current revisions arrive, either before the commit
        // (j_a at 2) or after it (j_a at 3). Either way the subscriber learns 3, once.
        for seen_by_the_store in [2, 3] {
            let notifier = InProcessNotifier::new();
            let subscription = notifier
                .subscribe(watching(&[Watch::One(journey("j_a"))]))
                .unwrap();
            notifier.publish(&journey("j_a"), revision(3));
            let mut revisions = current();
            revisions
                .journeys
                .insert("j_a".parse().unwrap(), revision(seen_by_the_store));
            subscription.seed(&revisions);
            assert_eq!(
                current_of(subscription.take(ms(0))),
                vec![tick(journey("j_a"), 3)],
                "store saw {seen_by_the_store}"
            );
            assert_eq!(subscription.take(SSE_COALESCING_INTERVAL), Take::Empty);
        }
    }

    #[test]
    fn ticks_coalesce_per_domain_and_are_handed_over_once_per_interval() {
        let notifier = InProcessNotifier::new();
        let subscription = notifier.subscribe(watching(&[Watch::Journeys])).unwrap();
        subscription.seed(&current());
        assert_eq!(current_of(subscription.take(ms(0))).len(), 2);

        notifier.publish(&journey("j_a"), revision(3));
        notifier.publish(&journey("j_a"), revision(4));
        notifier.publish(&journey("j_b"), revision(6));
        assert_eq!(
            subscription.take(ms(100)),
            Take::Wait {
                until: SSE_COALESCING_INTERVAL
            }
        );
        assert_eq!(
            ticks(subscription.take(SSE_COALESCING_INTERVAL)),
            vec![tick(journey("j_a"), 4), tick(journey("j_b"), 6)]
        );

        // Out of order or repeated, an older revision is never handed over again.
        notifier.publish(&journey("j_a"), revision(3));
        notifier.publish(&journey("j_b"), revision(6));
        assert_eq!(subscription.take(ms(1_000)), Take::Empty);
        notifier.publish(&journey("j_a"), revision(5));
        assert_eq!(
            ticks(subscription.take(ms(1_000))),
            vec![tick(journey("j_a"), 5)]
        );
    }

    #[test]
    fn a_journey_deleted_while_away_shows_as_gone_in_the_first_take() {
        // j_b was hard-deleted before this subscriber (re)connected, so the store no longer
        // lists it. The first take is complete: a kind watch that held j_b sees it absent, a
        // watch by name sees it at 0, and with nothing watched existing the take is complete
        // and empty rather than nothing.
        let mut after_delete = current();
        after_delete.journeys.remove(&"j_b".parse().unwrap());
        let nothing = Revisions::default();
        let cases: [(&Revisions, Vec<Watch>, Vec<Tick>); 3] = [
            (
                &after_delete,
                vec![Watch::Journeys],
                vec![tick(journey("j_a"), 2)],
            ),
            (
                &after_delete,
                vec![Watch::One(journey("j_b"))],
                vec![tick(journey("j_b"), 0)],
            ),
            (&nothing, vec![Watch::Routes, Watch::Proposals], Vec::new()),
        ];
        for (revisions, watches, expected) in cases {
            let notifier = InProcessNotifier::new();
            let subscription = notifier.subscribe(watching(&watches)).unwrap();
            subscription.seed(revisions);
            assert_eq!(
                current_of(subscription.take(ms(0))),
                expected,
                "{watches:?}"
            );
        }
    }

    #[test]
    fn a_subscriber_hears_only_what_it_watches() {
        let notifier = InProcessNotifier::new();
        let one = notifier
            .subscribe(watching(&[Watch::One(proposal("pr_x"))]))
            .unwrap();
        let routes = notifier.subscribe(watching(&[Watch::Routes])).unwrap();
        for (of, at) in [
            (journey("j_a"), 3),
            (route("vendor"), 2),
            (proposal("pr_y"), 1),
            (proposal("pr_x"), 5),
            (deployment(), 4),
        ] {
            notifier.publish(&of, revision(at));
        }
        assert_eq!(ticks(one.take(ms(0))), vec![tick(proposal("pr_x"), 5)]);
        assert_eq!(ticks(routes.take(ms(0))), vec![tick(route("vendor"), 2)]);
    }

    #[test]
    fn subscribers_are_limited_and_a_dropped_one_frees_its_place() {
        let notifier = InProcessNotifier::new();
        let max = usize::try_from(SSE_SUBSCRIBER_COUNT_MAX).unwrap();
        let mut held: Vec<_> = (0..max)
            .map(|_| notifier.subscribe(watching(&[Watch::Journeys])).unwrap())
            .collect();
        assert_eq!(notifier.subscriber_count(), max);
        assert!(notifier.subscribe(watching(&[Watch::Routes])).is_err());
        held.pop();
        assert_eq!(notifier.subscriber_count(), max - 1);
        let newcomer = notifier.subscribe(watching(&[Watch::Routes])).unwrap();
        notifier.publish(&route("vendor"), revision(2));
        assert_eq!(ticks(newcomer.take(ms(0))), vec![tick(route("vendor"), 2)]);
    }

    struct CountingWaker(AtomicU32);

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn waiting_for_a_tick_wakes_on_a_publish_it_watches() {
        let notifier = InProcessNotifier::new();
        let subscription = notifier
            .subscribe(watching(&[Watch::One(journey("j_a"))]))
            .unwrap();
        let woken = Arc::new(CountingWaker(AtomicU32::new(0)));
        let waker = Waker::from(woken.clone());
        let mut context = Context::from_waker(&waker);
        let mut ready = pin!(subscription.ready());
        assert_eq!(ready.as_mut().poll(&mut context), Poll::Pending);
        notifier.publish(&journey("j_b"), revision(2));
        assert_eq!(woken.0.load(Ordering::SeqCst), 0, "not watched");
        notifier.publish(&journey("j_a"), revision(2));
        assert_eq!(woken.0.load(Ordering::SeqCst), 1);
        assert_eq!(ready.as_mut().poll(&mut context), Poll::Ready(()));
    }

    #[test]
    fn an_empty_seeded_snapshot_is_ready_and_wakes_its_waiter() {
        let notifier = InProcessNotifier::new();
        let subscription = notifier.subscribe(watching(&[Watch::Routes])).unwrap();
        let woken = Arc::new(CountingWaker(AtomicU32::new(0)));
        let waker = Waker::from(woken.clone());
        let mut context = Context::from_waker(&waker);
        let mut ready = pin!(subscription.ready());
        assert_eq!(ready.as_mut().poll(&mut context), Poll::Pending);
        subscription.seed(&Revisions::default());
        assert_eq!(woken.0.load(Ordering::SeqCst), 1);
        assert_eq!(ready.as_mut().poll(&mut context), Poll::Ready(()));
        assert_eq!(subscription.take(ms(0)), Take::Current(Vec::new()));
        let mut after = pin!(subscription.ready());
        assert_eq!(after.as_mut().poll(&mut context), Poll::Pending, "taken");
    }
}
