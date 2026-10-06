//! The fixtures the in-browser host is seeded with on every load (ARCHITECTURE, Web UI:
//! in-browser host; fixtures/README.md), compiled in, and the seeding itself: each fixture's
//! route published as version 1, then its scenario's steps submitted through the service in
//! order, every fixture into one deployment.
//!
//! Each scenario is written for a fresh deployment at revision 0; seeded one after another,
//! a step that names a deployment revision names it counted from where its fixture started,
//! so seeding moves it by the deployment revision the fixture started at. Entity keys are
//! distinct across the fixtures, so nothing else collides.
//!
//! Cost: one service write per route and per step, a few dozen in all.

use cairn_engine::from_file;
use cairn_schema::{
    Actor, DraftSource, Email, Mutation, Mutations, Patch, PatchTarget, Revision, RouteFile,
    Scenario, ScenarioStep, SequentialKeys, Timestamp, from_yaml,
};
use cairn_service::{Call, DomainPatch, Service};
use cairn_store::{IdentityRecord, Store, UserRecord};

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

/// Every fixture, in the order they are seeded.
pub const FIXTURES: [Fixture; 4] = [
    Fixture {
        name: "bake-off",
        route: None,
        journey: include_str!("../../../fixtures/bake-off/journey.yaml"),
    },
    Fixture {
        name: "hiring-loop",
        route: Some(include_str!("../../../fixtures/hiring-loop/route.yaml")),
        journey: include_str!("../../../fixtures/hiring-loop/journey.yaml"),
    },
    Fixture {
        name: "product-launch",
        route: Some(include_str!("../../../fixtures/product-launch/route.yaml")),
        journey: include_str!("../../../fixtures/product-launch/journey.yaml"),
    },
    Fixture {
        name: "vendor-evaluation",
        route: Some(include_str!(
            "../../../fixtures/vendor-evaluation/route.yaml"
        )),
        journey: include_str!("../../../fixtures/vendor-evaluation/journey.yaml"),
    },
];

/// Why seeding stopped: a fixture that did not parse or a step the service did not commit,
/// which is a bug in the fixtures or the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeedError(pub String);

impl std::fmt::Display for SeedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "seeding the fixtures failed: {}", self.0)
    }
}

impl std::error::Error for SeedError {}

impl Fixture {
    /// Its journey scenario.
    ///
    /// # Errors
    ///
    /// When the file does not parse.
    pub fn scenario(&self) -> Result<Scenario, SeedError> {
        from_yaml(self.journey).map_err(|error| SeedError(format!("{}: {error}", self.name)))
    }

    /// The patch that creates its route and publishes the file as version 1, as every
    /// scenario expects (fixtures/README.md): the route, a draft opened for an import, its
    /// roles, kinds, default owner, and nodes, and the publish. None for a fixture with no
    /// route.
    ///
    /// # Errors
    ///
    /// When the file does not parse or build.
    pub fn route_patch(&self) -> Result<Option<Patch>, SeedError> {
        let Some(text) = self.route else {
            return Ok(None);
        };
        let failed = |error: &dyn std::fmt::Display| SeedError(format!("{}: {error}", self.name));
        let file: RouteFile = from_yaml(text).map_err(|error| failed(&error))?;
        let graph = from_file(&file, &mut SequentialKeys::default())
            .map_err(|violations| failed(&format_args!("{violations:?}")))?
            .into_document();
        let mut mutations = vec![
            Mutation::CreateRoute {
                name: file.name.clone(),
                description: file.description.clone(),
            },
            Mutation::OpenDraft {
                source: DraftSource::Import,
            },
        ];
        mutations.extend(
            graph
                .roles
                .values()
                .map(|role| Mutation::AddRole { role: role.clone() }),
        );
        mutations.extend(
            graph
                .participation_kinds
                .values()
                .map(|kind| Mutation::AddParticipationKind { kind: kind.clone() }),
        );
        if let Some(role) = &graph.default_owner {
            mutations.push(Mutation::SetDefaultOwner {
                role: Some(role.clone()),
            });
        }
        mutations.extend(
            graph
                .nodes
                .values()
                .map(|node| Mutation::AddNode { node: node.clone() }),
        );
        mutations.push(Mutation::PublishDraft);
        let id = format!("p_seed_{}", self.name.replace('-', "_"));
        Ok(Some(Patch {
            id: id
                .parse()
                .map_err(|error| failed(&format_args!("{error:?}")))?,
            target: PatchTarget::Route(file.route.clone()),
            base_revision: Revision::NONE,
            deployment_revision: None,
            mutations: Mutations::new(mutations).map_err(|error| failed(&error))?,
        }))
    }
}

/// Who publishes the fixtures' routes, and when: before any scenario starts.
fn author_call() -> Call {
    let parse = |text: &str| {
        text.parse()
            .unwrap_or_else(|error| panic!("{text}: {error:?}"))
    };
    Call {
        actor: Actor {
            user: parse("u_author"),
            agent: None,
        },
        now: "2026-09-01T12:00:00Z"
            .parse::<Timestamp>()
            .unwrap_or_else(|error| panic!("a fixed time: {error}")),
    }
}

/// Seeds every fixture into `service`, in [`FIXTURES`] order.
///
/// # Errors
///
/// When a fixture does not parse or the service does not commit a step.
pub async fn seed<S: Store>(service: &Service<S>) -> Result<(), SeedError> {
    for fixture in &FIXTURES {
        let start = service
            .deployment()
            .await
            .map_err(|error| SeedError(error.to_string()))?
            .revision;
        if let Some(patch) = fixture.route_patch()? {
            submit(service, &author_call(), patch, None).await?;
        }
        for step in fixture.scenario()?.steps.as_slice() {
            seed_step(service, step, start).await?;
        }
    }
    Ok(())
}

/// One scenario step, its deployment revision counted from `start`: a proposal's create or
/// apply through the proposal lifecycle (I6), anything else as a domain patch.
async fn seed_step<S: Store>(
    service: &Service<S>,
    step: &ScenarioStep,
    start: Revision,
) -> Result<(), SeedError> {
    let call = Call {
        actor: step.actor.clone(),
        now: step.at,
    };
    let failed = |error: &dyn std::fmt::Debug| SeedError(format!("{}: {error:?}", step.patch.id));
    let mut patch = step.patch.clone();
    if let Some(revision) = patch.deployment_revision {
        let moved = Revision::try_from(revision.get() + start.get()).map_err(|e| failed(&e))?;
        patch.deployment_revision = Some(moved);
    }
    match (&patch.target, patch.mutations.as_slice()) {
        (PatchTarget::Proposal { id, destination }, [Mutation::CreateProposal { proposal }]) => {
            service
                .create_proposal(&call, patch.id.clone(), destination, id, proposal.clone())
                .await
                .map_err(|error| failed(&error))?;
        }
        (
            PatchTarget::Journey(journey),
            [
                Mutation::ApplyProposal {
                    proposal,
                    reviewed_revision,
                },
            ],
        ) => {
            let destination = cairn_schema::Domain::Journey(journey.clone());
            service
                .apply_proposal(
                    &call,
                    patch.id.clone(),
                    &destination,
                    proposal,
                    *reviewed_revision,
                    Some(step.note.clone()),
                )
                .await
                .map_err(|error| failed(&error))?;
        }
        _ => submit(service, &call, patch, Some(step.note.clone())).await?,
    }
    Ok(())
}

/// Submits a domain patch, which must commit.
async fn submit<S: Store>(
    service: &Service<S>,
    call: &Call,
    patch: Patch,
    note: Option<cairn_schema::Markdown>,
) -> Result<(), SeedError> {
    let id = patch.id.clone();
    let submitted =
        DomainPatch::new(patch, note).map_err(|error| SeedError(format!("{id}: {error:?}")))?;
    service
        .patch(call, &submitted)
        .await
        .map(|_| ())
        .map_err(|error| SeedError(format!("{id}: {error:?}")))
}

/// The verified emails of the browser host's local identity: each fixture's lead, so the
/// viewer is one entity in every fixture (H3) and "mine", the owner factor, and the ranking
/// for the viewer show that lead's work.
pub const LOCAL_EMAILS: [&str; 4] = [
    "owner@example.org",
    "manager@example.org",
    "launch-lead@example.org",
    "lead@example.org",
];

/// Signs in the browser host's one local identity on `store`: user `u_local` through the
/// `local` method, verified for [`LOCAL_EMAILS`]. Who everything in the root is done as.
///
/// # Errors
///
/// When the store fails.
pub async fn sign_in_local<S: Store>(store: &S, at: Timestamp) -> Result<Actor, SeedError> {
    fn parse<T: std::str::FromStr<Err: std::fmt::Debug>>(text: &str) -> Result<T, SeedError> {
        text.parse()
            .map_err(|error| SeedError(format!("{text}: {error:?}")))
    }
    let user: cairn_schema::UserId = parse("u_local")?;
    let failed = |error: cairn_store::StoreError| SeedError(error.to_string());
    store
        .put_user(UserRecord {
            id: user.clone(),
            name: parse("Local user")?,
            created_at: at,
        })
        .await
        .map_err(failed)?;
    let verified_emails = LOCAL_EMAILS
        .iter()
        .map(|email| parse::<Email>(email))
        .collect::<Result<_, _>>()?;
    store
        .put_identity(IdentityRecord {
            provider: parse("local")?,
            subject: parse("local")?,
            user: user.clone(),
            verified_emails,
            linked_at: at,
        })
        .await
        .map_err(failed)?;
    Ok(Actor { user, agent: None })
}
