//! Pass 5's cost test at the limits (PRACTICES, Explicit limits; the validation ladder, rung
//! 3: cost tests budgeted in operations, not wall-clock time). On `generated::date_limits`
//! (`node_count_max` nodes at the depth limit), the topological order and the sweep read each
//! instant once and each edge at most three times: counted and released by the order, and
//! read once by the sweep from its dependent.

#[cfg(test)]
mod cost {
    use cairn_engine::derive;
    use cairn_engine::derive::EdgeSet;
    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::date_limits;
    use cairn_schema::Deployment;

    #[test]
    fn blocking_stays_linear_at_the_limits() {
        let graph = date_limits();
        let derived = derive(&graph, None, &derive_inputs(Deployment::default()));
        let dependencies = derived.dependencies();
        let instants = u64::try_from(dependencies.node_count() * 4).unwrap();
        let edges = u64::try_from(dependencies.edges(EdgeSet::Full).count()).unwrap();
        let operations = derived.blocking().operation_count();
        println!(
            "blocking at the limits: {instants} instants, {edges} edges; {operations} operations"
        );
        assert_eq!(dependencies.node_count(), 2_000, "the node limit");
        assert!(operations <= instants + 3 * edges, "{operations}");
    }
}
