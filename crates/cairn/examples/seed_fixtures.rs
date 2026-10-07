//! Seeds every fixture into a new database file, for the browser tests that run the real
//! binary over it (web/app/playwright.config.ts): `cargo run -p cairn --example
//! seed_fixtures -- <database>`.

#[path = "../tests/support/seed.rs"]
mod seed;

fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: seed_fixtures <database>")?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?
        .block_on(seed::seed_fixtures(std::path::Path::new(&path)))
}
