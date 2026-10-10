//! The date network's cost test at the limits (PRACTICES, Explicit limits; the validation
//! ladder: cost tests budgeted in operations, not wall-clock time). The graph is
//! `generated::date_limits`: `node_count_max` nodes at the depth limit with both date rules
//! on every node at `edge_count_per_node_max` sources each, which ties most instants into
//! one strongly connected component, the case Bellman-Ford pays for.

#[cfg(test)]
mod cost {
    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::date_limits;
    use cairn_engine::{check_plan, derive};
    use cairn_schema::Deployment;

    /// Operations each solve may spend at the limits, per constraint: the components found
    /// once, each constraint relaxed a handful of times. The worst case ARCHITECTURE states
    /// is one relaxation per constraint per instant; this budget holds the generated limits
    /// graph to far less.
    const OPERATIONS_PER_CONSTRAINT_MAX: u64 = 16;

    #[test]
    fn the_date_network_stays_within_its_operation_budget_at_the_limits() {
        let graph = date_limits();
        let derived = derive(&graph, None, &derive_inputs(Deployment::default()));
        let (instants, constraints) = derived.dates().network_size();
        let constraints = u64::try_from(constraints).unwrap();
        let checked = check_plan(&graph, &Deployment::default());
        assert!(checked.is_consistent());
        let plan = checked.operation_count();
        let execution = derived.dates().operation_count();
        println!(
            "date network at the limits: {instants} instants, {constraints} constraints; plan check {plan} operations; execution (both passes) {execution} operations"
        );
        assert!(
            constraints > 300_000,
            "the rules reach the constraint limit"
        );
        assert!(
            plan <= OPERATIONS_PER_CONSTRAINT_MAX * constraints,
            "{plan}"
        );
        assert!(
            execution <= 2 * OPERATIONS_PER_CONSTRAINT_MAX * constraints,
            "{execution}"
        );
    }
}
