//! Playing a fixture (fixtures/README.md) into a service: a fixture is a list of writes, each
//! with an actor and a time, and playing it submits them as the system under test would
//! receive them. The demo's seed and every harness that loads fixtures go through here, so
//! a proposal step is a proposal and a step is its actor's at its time, everywhere.

use cairn_engine::from_file;
use cairn_schema::{
    Actor, Domain, DraftSource, Mutation, Mutations, Patch, PatchTarget, Revision, RouteFile,
    ScenarioStep, SequentialKeys, Timestamp, from_yaml,
};

use crate::{Call, DomainPatch, Proposing, Service};

/// Why a fixture did not play: one that did not parse or a step the service did not commit,
/// which is a bug in the fixture or the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayError(pub String);

impl std::fmt::Display for PlayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "playing the fixtures failed: {}", self.0)
    }
}

impl std::error::Error for PlayError {}

/// The patch that creates a fixture's route and publishes its file as version 1, as every
/// scenario expects (fixtures/README.md): the route, a draft opened for an import, its roles,
/// kinds, default owner, and nodes, and the publish. `name` is the fixture's directory and
/// `text` its `route.yaml`.
///
/// # Errors
///
/// When the file does not parse or build.
pub fn route_patch(name: &str, text: &str) -> Result<Patch, PlayError> {
    let failed = |error: &dyn std::fmt::Display| PlayError(format!("{name}: {error}"));
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
    mutations.extend(cairn_engine::format::content(&graph));
    mutations.push(Mutation::PublishDraft {});
    let id = format!("p_seed_{}", name.replace('-', "_"));
    Ok(Patch {
        id: id
            .parse()
            .map_err(|error| failed(&format_args!("{error:?}")))?,
        target: PatchTarget::Route(file.route.clone()),
        base_revision: Revision::NONE,
        deployment_revision: None,
        mutations: Mutations::new(mutations).map_err(|error| failed(&error))?,
    })
}

/// Who publishes the fixtures' routes, and when: before any scenario starts.
///
/// # Panics
///
/// Never: the user and the time are constants.
#[must_use]
pub fn publisher() -> Call {
    Call {
        actor: Actor {
            user: "u_author"
                .parse()
                .unwrap_or_else(|error| panic!("a fixed user: {error:?}")),
            agent: None,
        },
        now: "2026-09-01T12:00:00Z"
            .parse::<Timestamp>()
            .unwrap_or_else(|error| panic!("a fixed time: {error}")),
    }
}

/// Publishes a fixture's route as version 1, as [`publisher`].
///
/// # Errors
///
/// When the file does not parse or build, or the service does not commit the write.
pub async fn publish_route(service: &Service, name: &str, text: &str) -> Result<(), PlayError> {
    submit(service, &publisher(), route_patch(name, text)?, None).await
}

/// Plays `steps`, or the first `first` of them, into `service` in order, each as its actor at
/// its time. A proposal's create goes through the proposal lifecycle and an apply of one
/// applies the proposal as its reviewer would; any other step is a domain patch.
///
/// A scenario is written for the deployment its people leave, at revision 1, so a step that
/// names a deployment revision names it counted from there: played into a deployment that has
/// moved since, the revision moves with it.
///
/// # Errors
///
/// When the service does not commit a step.
pub async fn play(
    service: &Service,
    steps: &[ScenarioStep],
    first: Option<usize>,
) -> Result<(), PlayError> {
    let at = service
        .deployment()
        .await
        .map_err(|error| PlayError(error.to_string()))?
        .revision;
    let start = at.get().saturating_sub(1);
    for step in steps.iter().take(first.unwrap_or(steps.len())) {
        play_step(service, step, start).await?;
    }
    Ok(())
}

/// One step, its deployment revision moved by `start`.
async fn play_step(service: &Service, step: &ScenarioStep, start: u32) -> Result<(), PlayError> {
    let call = Call {
        actor: step.actor.clone(),
        now: step.at,
    };
    let failed = |error: &dyn std::fmt::Debug| PlayError(format!("{}: {error:?}", step.patch.id));
    let mut patch = step.patch.clone();
    if let Some(revision) = patch.deployment_revision {
        let moved = Revision::try_from(revision.get() + start).map_err(|e| failed(&e))?;
        patch.deployment_revision = Some(moved);
    }
    match (&patch.target, patch.mutations.as_slice()) {
        (PatchTarget::Proposal { id, destination }, [Mutation::CreateProposal { proposal }]) => {
            // The draft as the write that makes it: its mutations against the destination
            // revision it was drafted at, titled by the note.
            let drafted = Patch {
                id: patch.id.clone(),
                target: destination.clone().into(),
                base_revision: proposal.destination_revision,
                deployment_revision: None,
                mutations: Mutations::new(proposal.mutations.as_slice().to_vec())
                    .map_err(|error| failed(&error))?,
            };
            let note = proposal.title.as_str().parse().map_err(|e| failed(&e))?;
            let drafted = DomainPatch::new(drafted, Some(note)).map_err(|error| failed(&error))?;
            let Some(proposing) =
                Proposing::named(Some(id.clone()), None).map_err(|e| failed(&e))?
            else {
                unreachable!("a proposal id was given");
            };
            service
                .propose(&call, &drafted, &proposing)
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
            service
                .apply_proposal(
                    &call,
                    patch.id.clone(),
                    &Domain::Journey(journey.clone()),
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
async fn submit(
    service: &Service,
    call: &Call,
    patch: Patch,
    note: Option<cairn_schema::Markdown>,
) -> Result<(), PlayError> {
    let id = patch.id.clone();
    let submitted =
        DomainPatch::new(patch, note).map_err(|error| PlayError(format!("{id}: {error:?}")))?;
    service
        .patch(call, &submitted)
        .await
        .map(|_| ())
        .map_err(|error| PlayError(format!("{id}: {error:?}")))
}
