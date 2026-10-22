//! The Rust client against a server in process (H5, H6): a stale patch the server applies
//! past what intervened when that cannot overlap it and answers stale when it can, and views
//! kept current by their subscriptions.
#![cfg(test)]

mod in_process {
    use cairn_api::client::{Client, Refused, Subscription, Tracker};
    use cairn_api::query::WatchName;
    use cairn_api::wire::{PatchAnswer, Tick};
    use cairn_schema::{Domain, JourneyId, Patch, Rejection, RevisionOf};

    use crate::api::support::{self, World};

    const GROUP: &str = "{key: n_g, id: g, kind: group, title: G}";
    const CHILD: &str = "{key: n_c, id: c, kind: action, title: C, parent: n_g}";
    const REMOVAL: &str = "- op: remove_node\n  removal: {node: n_g, descendants: [n_c]}\n";

    fn journey_patch(id: &str, base: u32, mutations: &str) -> Patch {
        support::patch(id, "{journey: j_tree}", base, mutations)
    }

    /// Creates `j_tree`, a group holding one action, at revision 1.
    async fn tree(client: &Client) {
        let mutations = format!(
            "- op: create_journey\n  name: Tree\n- op: add_node\n  node: {GROUP}\n- op: add_node\n  node: {CHILD}\n"
        );
        landed(
            client
                .submit(journey_patch("p_create", 0, &mutations), None)
                .await,
        );
    }

    fn landed<E: std::fmt::Debug>(answer: Result<PatchAnswer, Refused<E>>) -> PatchAnswer {
        answer.unwrap_or_else(|refused| panic!("not landed: {refused:#?}"))
    }

    fn rejected<E: std::fmt::Debug>(answer: Result<PatchAnswer, Refused<E>>) -> Rejection {
        match answer {
            Err(Refused::Rejected(rejection)) => rejection,
            other => panic!("not rejected: {other:#?}"),
        }
    }

    fn note(id: &str, key: &str, base: u32) -> Patch {
        let mutations =
            format!("- op: add_annotation\n  annotation: {{key: {key}, note: Seen.}}\n");
        journey_patch(id, base, &mutations)
    }

    /// H5: two people note the same journey from the same revision; the second is stale,
    /// what intervened cannot overlap it, and the server applies it at the new revision and
    /// says what it moved past.
    #[tokio::test]
    async fn a_non_overlapping_conflict_is_applied_over_by_the_server() {
        let world = World::start().await;
        let (ann, bob) = (world.client("ann"), world.client("bob"));
        tree(&ann).await;
        landed(bob.submit(note("p_bob", "a_bob", 1), None).await);
        let applied = landed(ann.submit(note("p_ann", "a_ann", 1), None).await);
        let PatchAnswer::Applied { receipt, over, .. } = applied else {
            panic!("applied, not {applied:#?}")
        };
        assert_eq!(receipt.revision.get(), 3);
        let over = over.expect("applied past bob's note");
        assert_eq!(
            over.conflicts
                .iter()
                .map(|conflict| (conflict.expected.get(), conflict.current.get()))
                .collect::<Vec<_>>(),
            [(1, 2)]
        );
        let journey = ann.journey(&"j_tree".parse().unwrap()).await.unwrap();
        assert_eq!(journey.graph.state.annotations.len(), 2);
        let answer = ann.submit(note("p_ann", "a_ann", 1), None).await.unwrap();
        assert!(
            matches!(answer, PatchAnswer::AlreadyApplied { .. }),
            "{answer:#?}"
        );
    }

    /// H5: a subtree removal drafted before an edit inside the subtree overlaps it, so the
    /// server answers stale rather than applying it.
    #[tokio::test]
    async fn an_overlapping_conflict_is_surfaced() {
        let world = World::start().await;
        let (ann, bob) = (world.client("ann"), world.client("bob"));
        tree(&ann).await;
        let edit = "- op: set_node_field\n  node: n_c\n  value: {title: Renamed}\n";
        landed(bob.submit(journey_patch("p_edit", 1, edit), None).await);
        let rejection = rejected(
            ann.submit(journey_patch("p_remove", 1, REMOVAL), None)
                .await,
        );
        let Rejection::Stale { conflicts, .. } = rejection else {
            panic!("stale, not {rejection:#?}")
        };
        assert_eq!(conflicts.len(), 1);
        let journey = ann.journey(&"j_tree".parse().unwrap()).await.unwrap();
        assert_eq!(journey.graph.nodes.len(), 2, "nothing was removed");
    }

    /// H5, A18: after an insertion beneath the subtree the removal would remove more than its
    /// author saw, so it would not apply at the new revision; the stale answer says it
    /// touches the whole journey and the server answers stale, never widening it.
    #[tokio::test]
    async fn a_removal_made_stale_beneath_it_is_surfaced_not_widened() {
        let world = World::start().await;
        let (ann, bob) = (world.client("ann"), world.client("bob"));
        tree(&ann).await;
        let insert =
            "- op: add_node\n  node: {key: n_d, id: d, kind: action, title: D, parent: n_g}\n";
        landed(bob.submit(journey_patch("p_insert", 1, insert), None).await);
        let rejection = rejected(
            ann.submit(journey_patch("p_remove", 1, REMOVAL), None)
                .await,
        );
        let Rejection::Stale { conflicts, .. } = rejection else {
            panic!("stale, not {rejection:#?}")
        };
        assert_eq!(conflicts.len(), 1);
        let journey = ann.journey(&"j_tree".parse().unwrap()).await.unwrap();
        assert_eq!(journey.graph.nodes.len(), 3, "nothing was removed");
    }

    /// A view: what it holds, and its subscription.
    struct View {
        client: Client,
        tracker: Tracker,
        subscription: Subscription,
    }

    impl View {
        /// Fetches the journey, then subscribes to it and the deployment.
        async fn open(client: Client, journey: &str) -> Self {
            let mut tracker = Tracker::new();
            let id: JourneyId = journey.parse().unwrap();
            let fetched = client.journey(&id).await.unwrap();
            tracker.fetched(&journey_of(journey), fetched.revision);
            let deployment = client.deployment().await.unwrap();
            tracker.fetched(&RevisionOf::Domain(Domain::Deployment), deployment.revision);
            let watching: Vec<WatchName> = [format!("journey:{journey}"), "deployment".to_owned()]
                .iter()
                .map(|name| name.parse().unwrap())
                .collect();
            let subscription = client.subscribe(&watching).await.unwrap();
            Self {
                client,
                tracker,
                subscription,
            }
        }

        /// Takes ticks until one asks for a refetch, refetches, and answers what it holds.
        async fn follow(&mut self) -> Tick {
            loop {
                let tick = self.subscription.next_tick().await.unwrap().unwrap();
                if self.tracker.ticked(&tick) {
                    let revision = match &tick.of {
                        RevisionOf::Domain(Domain::Journey(id)) => {
                            self.client.journey(id).await.unwrap().revision
                        }
                        _ => self.client.deployment().await.unwrap().revision,
                    };
                    self.tracker.fetched(&tick.of, revision);
                    return tick;
                }
            }
        }
    }

    fn journey_of(id: &str) -> RevisionOf {
        RevisionOf::Domain(Domain::Journey(id.parse().unwrap()))
    }

    /// H6: two people on one journey; each sees the other's commit and refetches once, and
    /// the current revisions each stream starts with ask for nothing they hold.
    #[tokio::test]
    async fn two_views_of_one_journey_stay_current() {
        let world = World::start().await;
        tree(&world.client("ann")).await;
        // Arriving gives a user an identity, which moves the deployment's revision: both
        // have arrived before either subscribes, so that is not an event of this test.
        world.client("bob").deployment().await.unwrap();
        let mut ann = View::open(world.client("ann"), "j_tree").await;
        let mut bob = View::open(world.client("bob"), "j_tree").await;
        landed(ann.client.submit(note("p_ann", "a_ann", 1), None).await);
        assert_eq!(bob.follow().await.of, journey_of("j_tree"));
        assert_eq!(bob.tracker.held(&journey_of("j_tree")).unwrap().get(), 2);
        let own = ann.follow().await;
        assert_eq!(
            own.revision.get(),
            2,
            "her own commit, which her view refetches too"
        );
        landed(bob.client.submit(note("p_bob", "a_bob", 2), None).await);
        let heard = ann.follow().await;
        assert_eq!((heard.of, heard.revision.get()), (journey_of("j_tree"), 3));
        let journey = ann
            .client
            .journey(&"j_tree".parse().unwrap())
            .await
            .unwrap();
        assert_eq!(journey.graph.state.annotations.len(), 2);
    }

    /// H6: a view that fetched before a commit and subscribed after it still learns the
    /// revision, from the current revisions its stream starts with.
    #[tokio::test]
    async fn a_late_subscriber_still_learns_the_revision() {
        let world = World::start().await;
        let ann = world.client("ann");
        tree(&ann).await;
        let mut tracker = Tracker::new();
        let fetched = ann.journey(&"j_tree".parse().unwrap()).await.unwrap();
        tracker.fetched(&journey_of("j_tree"), fetched.revision);
        landed(
            world
                .client("bob")
                .submit(note("p_bob", "a_bob", 1), None)
                .await,
        );
        let watching = ["journey:j_tree".parse().unwrap()];
        let mut subscription = ann.subscribe(&watching).await.unwrap();
        let first = subscription.next_tick().await.unwrap().unwrap();
        assert_eq!(first.revision.get(), 2);
        assert!(tracker.ticked(&first), "newer than what it fetched");
    }

    /// H6, E6: an identity merge moves the deployment revision, and a view watching the
    /// deployment hears it.
    #[tokio::test]
    async fn an_identity_merge_ticks_the_deployment() {
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let mut view = View::open(world.client("ann"), "j_vendor_eval").await;
        drop(ann);
        let merge = support::patch(
            "p_merge",
            "deployment",
            1,
            "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [who_e7h2jv3nxc4yaw6t]\n  journeys: {j_vendor_eval: 2}\n",
        );
        landed(view.client.submit(merge, None).await);
        let heard = view.follow().await;
        assert_eq!(
            (heard.of, heard.revision.get()),
            (RevisionOf::Domain(Domain::Deployment), 2)
        );
    }
}
