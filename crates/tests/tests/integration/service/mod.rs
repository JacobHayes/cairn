//! The tests of `cairn-service`: one module per file beside this one.

#[cfg(test)]
mod support;

mod composition;
mod in_flight;
mod property_write;
mod proposals;
mod reads;
mod routes;
mod writes;
