//! The fixtures `cairn demo` is seeded with (fixtures/README.md), compiled in, and the
//! seeding itself: the shared people, then each fixture's route published as version 1 and
//! its scenario played through the service (`cairn_service::play`), every fixture into one
//! deployment. The identities a scenario creates are distinct across the fixtures, so nothing
//! collides.
//!
//! Cost: one service write per route and per step, a few dozen in all.

use cairn_schema::{Email, Scenario, ScenarioStep, TimeZoneName, Timestamp, from_yaml};
use cairn_service::{DeploymentSettings, PlayError, Service};
use cairn_store::{LoginRecord, Store, UserRecord};

/// One fixture: its name, its route file if it has one, and its journey scenario.
#[derive(Clone, Copy, Debug)]
pub struct Fixture {
    /// The fixture's directory under `fixtures/`.
    pub name: &'static str,
    /// Its `route.yaml`, published as version 1 before the scenario runs.
    pub route: Option<&'static str>,
    /// Its `journey.yaml`.
    pub journey: &'static str,
}

/// The patch that creates the people every scenario starts from (`fixtures/people.yaml`).
const PEOPLE: &str = include_str!("../../../fixtures/people.yaml");

/// The deployment the fixtures record (`fixtures/deployment.yaml`).
const DEPLOYMENT: &str = include_str!("../../../fixtures/deployment.yaml");

// Every fixture, in name order (the order they are seeded); the `fixtures/` directory owns
// the list (build.rs).
include!(concat!(env!("OUT_DIR"), "/fixtures.rs"));

impl Fixture {
    /// Its journey scenario.
    ///
    /// # Errors
    ///
    /// When the file does not parse.
    pub fn scenario(&self) -> Result<Scenario, PlayError> {
        from_yaml(self.journey).map_err(|error| PlayError(format!("{}: {error}", self.name)))
    }
}

/// The time zone the fixtures declare, which every deployment that loads them runs in.
///
/// # Errors
///
/// When the file does not parse.
pub fn time_zone() -> Result<TimeZoneName, PlayError> {
    cairn_service::time_zone(DEPLOYMENT)
}

/// The settings of a deployment that loads the fixtures: their time zone, with its rules from
/// the system's zone database.
///
/// # Errors
///
/// When the file does not parse or the system's database does not know the zone.
pub fn settings() -> Result<DeploymentSettings, PlayError> {
    let name = time_zone()?;
    let zone = jiff::tz::TimeZone::get(name.as_str())
        .map_err(|error| PlayError(format!("{}: {error}", name.as_str())))?;
    Ok(DeploymentSettings::new(name, zone))
}

/// Seeds every fixture into `service`, in [`FIXTURES`] order.
///
/// # Errors
///
/// When a fixture does not parse or the service does not commit a step.
pub async fn seed(service: &Service) -> Result<(), PlayError> {
    let people: ScenarioStep =
        from_yaml(PEOPLE).map_err(|error| PlayError(format!("people: {error}")))?;
    cairn_service::play(service, &[people], None).await?;
    for fixture in &FIXTURES {
        if let Some(route) = fixture.route {
            cairn_service::publish_route(service, fixture.name, route).await?;
        }
        cairn_service::play(service, fixture.scenario()?.steps.as_slice(), None).await?;
    }
    Ok(())
}

/// The users the fixtures' events name (the routes' publisher and the scenarios' actors),
/// each but the publisher with one login, named for the person and verified for their email,
/// so a history says who rather than an id.
///
/// # Errors
///
/// When the store fails.
pub async fn put_fixture_users(store: &dyn Store, at: Timestamp) -> Result<(), PlayError> {
    const USERS: [(&str, &str, Option<&str>); 4] = [
        ("u_author", "Route author", None),
        ("u_ana", "Ana Ruiz", Some("ana@example.org")),
        ("u_ben", "Ben Okafor", Some("ben@example.org")),
        ("u_gus", "Gus Lindqvist", Some("gus@example.org")),
    ];
    fn parse<T: std::str::FromStr<Err: std::fmt::Debug>>(text: &str) -> Result<T, PlayError> {
        text.parse()
            .map_err(|error| PlayError(format!("{text}: {error:?}")))
    }
    let failed = |error: cairn_store::StoreError| PlayError(error.to_string());
    for (id, name, email) in USERS {
        let user: cairn_schema::UserId = parse(id)?;
        store
            .put_user(UserRecord {
                id: user.clone(),
                created_at: at,
            })
            .await
            .map_err(failed)?;
        if let Some(email) = email {
            store
                .put_login(LoginRecord {
                    provider: parse("fixture")?,
                    subject: parse(id)?,
                    user,
                    name: Some(parse(name)?),
                    verified_emails: [parse::<Email>(email)?].into_iter().collect(),
                    linked_at: at,
                })
                .await
                .map_err(failed)?;
        }
    }
    Ok(())
}
