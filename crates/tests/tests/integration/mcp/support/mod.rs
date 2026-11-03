//! What the tool set's tests share: the tools over a fresh memory store, a clock the test
//! sets, actors, and set-up written through the service: the fixtures' route published and
//! the vendor evaluation started.
#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use cairn_auth::Clock;
use cairn_mcp::{ToolError, ToolSet};
use cairn_schema::{Actor, Patch, Timestamp};
use cairn_service::{
    Call, Capabilities, DeploymentSettings, DomainPatch, McpAccess, Parts, Service,
};
use cairn_store::{InProcessNotifier, MemoryStore};
use jiff::tz::{Offset, TimeZone};
use serde_json::Value;

/// The tool set over a fresh memory store, the service beneath it, and the clock its calls
/// read.
pub struct World {
    pub tools: ToolSet,
    pub service: Service,
    now: Arc<AtomicI64>,
}

impl World {
    /// The tools over an empty deployment, at the vendor evaluation's first step.
    pub fn new() -> Self {
        let zone = TimeZone::fixed(Offset::constant(-5));
        let settings = DeploymentSettings::new("Etc/GMT+5".parse().unwrap(), zone);
        let service = Service::new(Parts {
            store: Arc::new(MemoryStore::new()),
            notifier: Arc::new(InProcessNotifier::new()),
            settings,
            capabilities: Capabilities {
                auth: Vec::new(),
                assistant: false,
                mcp: McpAccess::closed("http://127.0.0.1/api/mcp"),
                version: "dev".to_owned(),
            },
        });
        let now = Arc::new(AtomicI64::new(0));
        let seconds = Arc::clone(&now);
        let clock =
            Clock::from_fn(move || Timestamp::from_second(seconds.load(Ordering::SeqCst)).unwrap());
        let tools = Self {
            tools: ToolSet::new(service.clone(), clock),
            service,
            now,
        };
        tools.set_clock("2026-10-01T14:00:00Z");
        tools
    }

    /// Sets the clock to `at`, an RFC 3339 time.
    pub fn set_clock(&self, at: &str) {
        let at: Timestamp = at.parse().unwrap();
        self.now.store(at.as_second(), Ordering::SeqCst);
    }

    /// Calls `name` as `actor`. Arguments a call succeeds with must match the tool's listed
    /// input schema: a listing that refuses what the tool takes misleads every agent.
    pub async fn call(
        &self,
        actor: &Actor,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        let answer = self.tools.call(actor, name, arguments.clone()).await;
        if answer.is_ok() {
            assert_listed(name, &arguments);
        }
        answer
    }

    /// Calls `name` as `actor`, which must succeed.
    pub async fn ok(&self, actor: &Actor, name: &str, arguments: Value) -> Value {
        match self.call(actor, name, arguments).await {
            Ok(output) => output,
            Err(error) => panic!("{name}: {error}"),
        }
    }

    /// Applies `patch` as `actor` through the service: set-up that is not what a test checks.
    pub async fn patch(&self, actor: &Actor, patch: Patch) {
        let call = Call {
            actor: actor.clone(),
            now: Timestamp::from_second(self.now.load(Ordering::SeqCst)).unwrap(),
        };
        let patch = DomainPatch::new(patch, None).unwrap();
        self.service.patch(&call, &patch).await.unwrap();
    }

    /// Publishes a fixture's route as version 1.
    pub async fn publish(&self, actor: &Actor, fixture: &str) {
        self.patch(actor, publish_fixture_route(fixture)).await;
    }

    /// Plays the vendor evaluation's first step: the shared people created, its route
    /// published, and `j_vendor_eval` started from its version 1, at revision 1.
    pub async fn vendor_journey(&self) {
        self.vendor_after(1).await;
    }

    /// Plays the vendor evaluation's scenario's first `steps` steps, each as its actor at its
    /// time, the clock left at the last: the journey at revision `steps`.
    pub async fn vendor_after(&self, steps: usize) {
        self.fixture_after("vendor-evaluation", Some(steps)).await;
    }

    /// Plays the fixture `name`'s scenario, its first `steps` steps or all of them, proposals
    /// included, the clock left at the last step: the journey it builds.
    pub async fn fixture_after(&self, name: &str, steps: Option<usize>) -> String {
        if let Some(at) = crate::fixtures::play_fixture(&self.service, name, steps).await {
            self.now.store(at.as_second(), Ordering::SeqCst);
        }
        crate::fixtures::scenario(name).journey.to_string()
    }
}

/// Fails unless `arguments` match the input schema the tool `name` is listed with.
pub fn assert_listed(name: &str, arguments: &Value) {
    let Some(tool) = ToolSet::definitions()
        .into_iter()
        .find(|tool| tool.name == name)
    else {
        return;
    };
    let schema = Value::Object(tool.input_schema);
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(arguments)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{name} took arguments its listed schema refuses: {errors:#?}"
    );
}

/// A user acting directly.
pub fn user(name: &str) -> Actor {
    Actor {
        user: name.parse().unwrap(),
        agent: None,
    }
}

/// An agent acting for a user (H2, I7).
pub fn agent(name: &str, user: &str) -> Actor {
    Actor {
        user: user.parse().unwrap(),
        agent: Some(name.parse().unwrap()),
    }
}

pub use crate::fixtures::{fixtures_root, publish_fixture_route};
