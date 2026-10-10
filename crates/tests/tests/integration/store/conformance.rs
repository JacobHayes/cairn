//! The store conformance suite against the memory backend, the reference.

mod conformance {
    cairn_store::conformance_suite!(
        cairn_store::conformance::Memory,
        cairn_store::conformance::block_on
    );
}
