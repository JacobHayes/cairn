//! The domain document read (ARCHITECTURE, HTTP API: `GET /journeys/{id}/document`; Terms:
//! Domain document): what the browser derives locally, packaged by the engine.

use cairn_engine::project;
use cairn_schema::{DomainDocument, JourneyId};
use cairn_store::Store;

use crate::{Call, Service, ServiceError};

impl<S: Store> Service<S> {
    /// A journey's domain document for `call`'s user, or none: its graph and state, the
    /// derive inputs at `call`'s today in the deployment's zone (A9) with the user's
    /// entities as the viewer (H3) and the current deployment context (E4, E6), and the
    /// engine version. Nothing derived is in it.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn document(
        &self,
        call: &Call,
        id: &JourneyId,
    ) -> Result<Option<DomainDocument>, ServiceError> {
        let Some(journey) = self.journey(id).await? else {
            return Ok(None);
        };
        // One deployment read serves both the viewer's entities and the context, so they
        // agree on its revision.
        let identities = self.store.identities_of(&call.actor.user).await?;
        let deployment = self.deployment().await?;
        let viewer = crate::viewer::entities_of(&identities, &deployment);
        let today = self.settings.today(call.now);
        let inputs = self.settings.derive_inputs(today, viewer, deployment);
        Ok(Some(project::document(&journey, &inputs)))
    }
}
