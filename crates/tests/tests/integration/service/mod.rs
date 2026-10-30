//! The tests of `cairn-service`: one module per file beside this one.

#[cfg(test)]
mod support;

mod arrival;
mod composition;
mod in_flight;
mod property_write;
mod proposals;
mod reads;
mod routes;
mod time_of_day;
mod writes;
