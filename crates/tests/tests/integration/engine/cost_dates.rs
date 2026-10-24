//! The date network's cost test at the limits (PRACTICES, Explicit limits; the validation
//! ladder: cost tests budgeted in operations, not wall-clock time). The graph is
//! `generated::date_limits`: `node_count_max` nodes at the depth limit with both date rules
//! on every node at `edge_count_per_node_max` sources each, which ties most instants into
//! one strongly connected component, the case Bellman-Ford pays for. The same graph with early
//! work holds the execution layer's one addition, the stage openings that do not gate, to the
//! same budgets.

#[cfg(test)]
mod cost {
    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::{date_limits, date_limits_early};
    use cairn_engine::{check_plan, derive};
    use cairn_schema::Deployment;

    /// Operations each solve may spend at the limits, per constraint: the components found
    /// once, each constraint relaxed a handful of times. The worst case ARCHITECTURE states
    /// is one relaxation per constraint per instant; this budget holds the generated limits
    /// graph to far less.
    const OPERATIONS_PER_CONSTRAINT_MAX: u64 = 16;

    /// The most constraints the module documentation of the date network allows at the node
    /// limit: the plan's, about 370,000, and the openings that do not gate, at most nodes
    /// times depth (32,000).
    const CONSTRAINTS_MAX: u64 = 370_000 + 32_000;

    /// The most memory the network may hold at the limits: the constraint bound at 44 bytes
    /// each (the constraint and its two adjacency slots), 18 MB, with room for the slot
    /// tables.
    const NETWORK_BYTES_MAX: usize = 20_000_000;

    #[test]
    fn the_date_network_stays_within_its_operation_budget_at_the_limits() {
        within_budget("date network at the limits", &date_limits(), 300_000);
    }

    /// The same graph with early work: openings that do not gate, and work started or
    /// finished ahead of what it waits on.
    #[test]
    fn early_work_stays_within_the_operation_budget_at_the_limits() {
        within_budget(
            "date network with early work",
            &date_limits_early(),
            300_000,
        );
    }

    fn within_budget(name: &str, graph: &cairn_engine::Graph, constraints_least: u64) {
        let derived = derive(graph, None, &derive_inputs(Deployment::default()));
        let (instants, constraints) = derived.dates().network_size();
        let constraints = u64::try_from(constraints).unwrap();
        let checked = check_plan(graph, &Deployment::default());
        assert!(checked.is_consistent());
        let plan = checked.operation_count();
        let execution = derived.dates().operation_count();
        let bytes = derived.dates().network_bytes();
        println!(
            "{name}: {instants} instants, {constraints} constraints, {bytes} bytes; plan check {plan} operations; execution (both passes) {execution} operations"
        );
        assert!(
            constraints > constraints_least,
            "the graph reaches the constraint count it is built for"
        );
        assert!(constraints <= CONSTRAINTS_MAX, "{constraints}");
        assert!(bytes <= NETWORK_BYTES_MAX, "{bytes}");
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
