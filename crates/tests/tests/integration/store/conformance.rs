//! The store conformance suite against the memory backend, the reference (rung 4).

mod conformance {
    cairn_store::conformance_suite!(
        cairn_store::conformance::Memory,
        cairn_store::conformance::block_on
    );
}
