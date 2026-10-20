//! The code step of migration 0011: the JSON the old names and keys live in. SQL cannot tell
//! a tag or a key from text a person wrote, so every stored shape is walked along the schema's
//! own structure, and only the positions that hold a tag, a field name or a key are changed.
//! Text a person wrote (a text answer, a title, a note, a choice id, the word "entity" in any
//! of them) is never touched, even when it equals an old key. The one map keyed by what a
//! person chose, a conflict resolution's `map_choices.map`, is never entered.
//!
//! Whether a compared value is an identity key depends on the decision it reads, so each value
//! is read in the scope it belongs to: a table row in its own graph (known exactly from its
//! `nodes` rows), a proposal in its destination's graph changed by the proposal's own nodes,
//! and an event in its log as the log's earlier events made it, with the nodes of the event's
//! own patch known first (a condition may name a decision added a step later). One scope never
//! changes another's.
//!
//! An identity's new key is `who_` and sixteen base32 characters of the SHA-256 of the old
//! key: the same on a rerun, unrelated to the name, and checked for collisions. The step
//! writes `identity_key_map` for the statements after it and rewrites, in place, the JSON
//! columns: `answers.value`, `nodes.relevant_when`, `proposals.mutations`, `proposals.items`,
//! `events.subject`, `events.delta` and `patch_receipts.deployment_touched`. Left as they are,
//! each for a reason: `patch_receipts.proposals` (proposal ids and revisions),
//! `insertions.roles`, `.kinds` and `.nodes` (maps of role, kind and node keys),
//! `oauth_transient.payload` (transient sign-in state, no identity) and the assistant's
//! `conversation_messages` (a transcript of what was said). The receipts' content hashes stay:
//! a patch resent from before the upgrade carries op names that no longer parse, so it is
//! refused as invalid and never mistaken for another.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest, Sha256};
use turso::Connection;

use crate::sql::{SqlError, execute, rows, text};

const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// The new key of the identity `old` named.
fn new_key(old: &str) -> String {
    let digest = Sha256::digest(old.as_bytes());
    let mut bits = 0u32;
    let mut have = 0u32;
    let mut out = String::from("who_");
    for byte in &digest {
        bits = (bits << 8) | u32::from(*byte);
        have += 8;
        while have >= 5 && out.len() < 4 + 16 {
            have -= 5;
            out.push(char::from(ALPHABET[((bits >> have) & 31) as usize]));
        }
    }
    out
}

/// Whether `text` has the shape an old identity key had.
fn is_old_key(text: &str) -> bool {
    text.strip_prefix("e_").is_some_and(|body| {
        body.starts_with(|first: char| first.is_ascii_lowercase() || first.is_ascii_digit())
            && body.chars().all(|each| {
                each.is_ascii_lowercase() || each.is_ascii_digit() || each == '_' || each == '-'
            })
    })
}

/// The decisions whose answers are identities, by the graph's owner (a journey or a route).
type Typed = BTreeMap<String, BTreeSet<String>>;

/// What the walk of one stored value knows: where it is, and the keys it has met.
struct Walk<'a> {
    owner: String,
    typed: &'a mut Typed,
    keys: &'a mut BTreeSet<String>,
}

/// The single-entry object `{tag: payload}` an externally tagged enum is stored as.
fn tagged(value: &mut Value) -> Option<(&str, &mut Value)> {
    let object = value.as_object_mut()?;
    if object.len() != 1 {
        return None;
    }
    let (tag, payload) = object.iter_mut().next()?;
    Some((tag.as_str(), payload))
}

/// Renames the tag of the externally tagged `value` when it is `from`.
fn retag(value: &mut Value, from: &str, to: &str) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    if object.len() == 1
        && let Some(payload) = object.remove(from)
    {
        object.insert(to.to_owned(), payload);
        return true;
    }
    false
}

impl Walk<'_> {
    /// A key at a position that holds an identity key.
    fn key(&mut self, value: &mut Value) {
        if let Value::String(old) = value
            && is_old_key(old)
        {
            self.keys.insert(old.clone());
            *old = new_key(old);
        }
    }

    fn keys_in(&mut self, value: &mut Value) {
        if let Value::Array(each) = value {
            for one in each {
                self.key(one);
            }
        }
    }

    /// An `AnswerValue`.
    fn answer(&mut self, value: &mut Value) {
        if retag(value, "entity", "identity")
            && let Some((_, payload)) = tagged(value)
        {
            self.key(payload);
        } else if retag(value, "entity_list", "identity_list")
            && let Some((_, payload)) = tagged(value)
        {
            self.keys_in(payload);
        }
    }

    /// A `ParticipationSource`: a role's key, or the identities written as a list.
    fn source(&mut self, value: &mut Value) {
        self.keys_in(value);
    }

    /// An identity as stored: `{key, name, emails}`.
    fn identity(&mut self, value: &mut Value) {
        if let Some(key) = value.get_mut("key") {
            self.key(key);
        }
    }

    /// A condition tree. A compared value is an identity key only where the decision it
    /// reads is an identity decision (the engine's own rule); everything else is text.
    fn clause(&mut self, value: &mut Value) {
        let Some((operator, operand)) = tagged(value) else {
            return;
        };
        match operator {
            "equals" | "not_equals" | "contains" => {
                if self.reads_identities(operand)
                    && let Some(compared) = operand.get_mut("value")
                {
                    self.key(compared);
                }
            }
            "in" => {
                if self.reads_identities(operand)
                    && let Some(compared) = operand.get_mut("values")
                {
                    self.keys_in(compared);
                }
            }
            "all" | "any" => {
                if let Value::Array(each) = operand {
                    for one in each {
                        self.clause(one);
                    }
                }
            }
            "not" => self.clause(operand),
            _ => {}
        }
    }

    fn reads_identities(&self, operand: &Value) -> bool {
        let Some(decision) = operand.get("decision").and_then(Value::as_str) else {
            return false;
        };
        self.typed
            .get(&self.owner)
            .is_some_and(|decisions| decisions.contains(decision))
    }

    /// Notes whether the node `value` is an identity decision, in this scope, without
    /// rewriting it.
    fn learn_node(&mut self, value: &Value) {
        let (Some(key), Some(answer_type)) = (
            value.get("key").and_then(Value::as_str),
            value.get("answer_type").and_then(Value::as_str),
        ) else {
            return;
        };
        let decisions = self.typed.entry(self.owner.clone()).or_default();
        if matches!(
            answer_type,
            "entity" | "entity_list" | "identity" | "identity_list"
        ) {
            decisions.insert(key.to_owned());
        } else {
            decisions.remove(key);
        }
    }

    /// The nodes a proposal's mutations add or replace, in order.
    fn learn_mutations(&mut self, value: &Value) {
        let Value::Array(each) = value else {
            return;
        };
        for mutation in each {
            if matches!(
                mutation.get("op").and_then(Value::as_str),
                Some("add_node" | "replace_node")
            ) && let Some(node) = mutation.get("node")
            {
                self.learn_node(node);
            }
        }
    }

    /// The nodes an event's writes put, in order.
    fn learn_delta(&mut self, value: &Value) {
        let Value::Array(each) = value else {
            return;
        };
        for write in each {
            if let Some(node) = write.pointer("/put/graph/record/node") {
                self.learn_node(node);
            }
        }
    }

    /// A node as authored or stored.
    fn node(&mut self, value: &mut Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        if let Some(Value::String(answer_type)) = object.get_mut("answer_type") {
            match answer_type.as_str() {
                "entity" => "identity".clone_into(answer_type),
                "entity_list" => "identity_list".clone_into(answer_type),
                _ => {}
            }
        }
        if let Some(condition) = object.get_mut("relevant_when") {
            self.clause(condition);
        }
        if let Some(Value::Object(participations)) = object.get_mut("participations") {
            for source in participations.values_mut() {
                self.source(source);
            }
        }
    }

    /// A `NodeFieldValue`.
    fn node_field(&mut self, value: &mut Value) {
        if let Some((field, payload)) = tagged(value)
            && field == "relevant_when"
        {
            self.clause(payload);
        }
    }

    /// A violation: its code and limit names, where it is, what it relates to, its sentence.
    fn violation(&mut self, value: &mut Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        for field in ["code", "limit"] {
            if let Some(Value::String(name)) = object.get_mut(field) {
                *name = name.replace("entity", "identity");
            }
        }
        if let Some(subject) = object.get_mut("at").and_then(|at| at.get_mut("subject")) {
            self.subject(subject);
        }
        if let Some(Value::Array(related)) = object.get_mut("related") {
            for subject in related {
                self.subject(subject);
            }
        }
        if let Some(Value::String(message)) = object.get_mut("message") {
            *message = self.reword(message);
        }
    }

    /// A sentence from a fixed template: the words entity and entities, and any old key.
    fn reword(&mut self, message: &str) -> String {
        let mut out = String::new();
        let mut word = String::new();
        let finish = |word: &mut String, out: &mut String, walk: &mut Self| {
            match word.as_str() {
                "entity" => out.push_str("identity"),
                "entities" => out.push_str("identities"),
                old if is_old_key(old) => {
                    walk.keys.insert(old.to_owned());
                    out.push_str(&new_key(old));
                }
                other => out.push_str(other),
            }
            word.clear();
        };
        for each in message.chars() {
            if each.is_ascii_alphanumeric() || each == '_' || each == '-' {
                word.push(each);
            } else {
                finish(&mut word, &mut out, self);
                out.push(each);
            }
        }
        finish(&mut word, &mut out, self);
        out
    }

    /// A `Subject`.
    fn subject(&mut self, value: &mut Value) {
        if retag(value, "entity", "identity")
            && let Some((_, payload)) = tagged(value)
        {
            self.key(payload);
        }
    }

    /// A `RecordKey`.
    fn record_key(&mut self, value: &mut Value) {
        if (retag(value, "entity", "identity") || retag(value, "entity_alias", "identity_alias"))
            && let Some((_, payload)) = tagged(value)
        {
            self.key(payload);
        }
    }

    /// A role reference of a conflict.
    fn role_reference(&mut self, value: &mut Value) {
        if let Some((tag, payload)) = tagged(value)
            && tag == "fill"
            && let Some(object) = payload.as_object_mut()
            && let Some(mut identities) = object.remove("entities")
        {
            self.keys_in(&mut identities);
            object.insert("identities".to_owned(), identities);
        }
    }

    fn conflict(&mut self, value: &mut Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        match object.get("about").and_then(Value::as_str) {
            Some("answer") => {
                if let Some(answer) = object.get_mut("answer") {
                    self.answer(answer);
                }
            }
            Some("role") => {
                if let Some(Value::Array(references)) = object.get_mut("references") {
                    for reference in references {
                        self.role_reference(reference);
                    }
                }
                if let Some(members) = object.get_mut("members") {
                    self.keys_in(members);
                }
            }
            Some("kind") => {
                if let Some(Value::Object(references)) = object.get_mut("references") {
                    for source in references.values_mut() {
                        self.source(source);
                    }
                }
            }
            Some("field") => {
                for side in ["journey", "route"] {
                    if let Some(field) = object.get_mut(side) {
                        self.node_field(field);
                    }
                }
            }
            Some("participation") => {
                for side in ["journey", "route"] {
                    if let Some(source) = object.get_mut(side) {
                        self.source(source);
                    }
                }
            }
            Some("shape") => {
                for side in ["journey", "route"] {
                    if let Some(node) = object.get_mut(side) {
                        self.node(node);
                    }
                }
            }
            _ => {}
        }
    }

    /// A `ReviewItem`.
    fn item(&mut self, value: &mut Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        match object.get("item").and_then(Value::as_str) {
            Some("conflict") => {
                if let Some(conflict) = object.get_mut("conflict") {
                    self.conflict(conflict);
                }
            }
            Some("participation") => {
                if let Some(mut identity) = object.remove("entity") {
                    self.key(&mut identity);
                    object.insert("identity".to_owned(), identity);
                }
            }
            Some("violation") => {
                if let Some(violation) = object.get_mut("violation") {
                    self.violation(violation);
                }
            }
            _ => {}
        }
    }

    /// A `Mutation`.
    fn mutation(&mut self, value: &mut Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        let Some(Value::String(op)) = object.get("op") else {
            return;
        };
        let op = op.clone();
        let renamed = match op.as_str() {
            "create_entity" => Some("create_identity"),
            "edit_entity" => Some("edit_identity"),
            "merge_entities" => Some("merge_identities"),
            _ => None,
        };
        if let Some(name) = renamed {
            object.insert("op".to_owned(), Value::String(name.to_owned()));
        }
        match op.as_str() {
            "create_entity" | "edit_entity" => {
                if let Some(mut identity) = object.remove("entity") {
                    self.identity(&mut identity);
                    object.insert("identity".to_owned(), identity);
                }
            }
            "merge_entities" => {
                for field in ["survivor", "merged"] {
                    if let Some(key) = object.get_mut(field) {
                        self.key(key);
                    }
                }
            }
            "add_node" | "replace_node" => {
                if let Some(node) = object.get_mut("node") {
                    self.node(node);
                }
            }
            "set_node_field" => {
                if let Some(field) = object.get_mut("value") {
                    self.node_field(field);
                }
            }
            "set_participation" => {
                if let Some(source) = object.get_mut("source") {
                    self.source(source);
                }
            }
            "answer" => {
                if let Some(answer) = object.get_mut("value") {
                    self.answer(answer);
                }
            }
            "fill_role" => {
                if let Some(mut identities) = object.remove("entities") {
                    self.keys_in(&mut identities);
                    object.insert("identities".to_owned(), identities);
                }
            }
            "create_proposal" | "edit_proposal" => {
                if let Some(draft) = object.get_mut("proposal") {
                    self.draft(draft);
                }
            }
            _ => {}
        }
    }

    fn mutations(&mut self, value: &mut Value) {
        if let Value::Array(each) = value {
            for one in each {
                self.mutation(one);
            }
        }
    }

    fn items(&mut self, value: &mut Value) {
        if let Value::Array(each) = value {
            for one in each {
                self.item(one);
            }
        }
    }

    /// A `ProposalDraft`.
    fn draft(&mut self, value: &mut Value) {
        if let Some(mutations) = value.get_mut("mutations") {
            self.mutations(mutations);
        }
        if let Some(items) = value.get_mut("items") {
            self.items(items);
        }
    }

    /// A `GraphRecord`.
    fn graph_record(&mut self, value: &mut Value) {
        let Some((tag, payload)) = tagged(value) else {
            return;
        };
        match tag {
            "node" => self.node(payload),
            "node_field" => {
                if let Some(field) = payload.get_mut("value") {
                    self.node_field(field);
                }
            }
            "participation" => {
                if let Some(source) = payload.get_mut("source") {
                    self.source(source);
                }
            }
            "answer" => {
                if let Some(answer) = payload.get_mut("value") {
                    self.answer(answer);
                }
            }
            "role_fill" => {
                if let Some(object) = payload.as_object_mut()
                    && let Some(mut identities) = object.remove("entities")
                {
                    self.keys_in(&mut identities);
                    object.insert("identities".to_owned(), identities);
                }
            }
            _ => {}
        }
    }

    /// A `Record`.
    fn record(&mut self, value: &mut Value) {
        if retag(value, "entity", "identity") {
            if let Some((_, payload)) = tagged(value) {
                self.identity(payload);
            }
            return;
        }
        if retag(value, "entity_alias", "identity_alias") {
            if let Some((_, payload)) = tagged(value)
                && let Some(object) = payload.as_object_mut()
            {
                if let Some(alias) = object.get_mut("alias") {
                    self.key(alias);
                }
                if let Some(mut identity) = object.remove("entity") {
                    self.key(&mut identity);
                    object.insert("identity".to_owned(), identity);
                }
            }
            return;
        }
        let Some((tag, payload)) = tagged(value) else {
            return;
        };
        match tag {
            "graph" => {
                if let Some(record) = payload.get_mut("record") {
                    self.graph_record(record);
                }
            }
            "proposal" => {
                if let Some(draft) = payload.get_mut("draft") {
                    // A candidate: the log's decisions changed by the proposal's own nodes, for
                    // this one value only.
                    let saved = self.typed.get(&self.owner).cloned();
                    if let Some(mutations) = draft.get("mutations").cloned() {
                        self.learn_mutations(&mutations);
                    }
                    self.draft(draft);
                    if let Some(saved) = saved {
                        self.typed.insert(self.owner.clone(), saved);
                    }
                }
            }
            _ => {}
        }
    }

    /// A `Write`.
    fn write(&mut self, value: &mut Value) {
        let Some((tag, payload)) = tagged(value) else {
            return;
        };
        match tag {
            "put" => self.record(payload),
            "remove" => self.record_key(payload),
            _ => {}
        }
    }
}

/// The graph row a log or a proposal's destination is about: a journey's own, or a route's
/// draft (a published version is a graph of its own, never a log's).
fn graph_of_log(kind: &str, id: Option<&str>) -> String {
    match (kind, id) {
        ("journey", Some(id)) => format!("journey/{id}"),
        ("route", Some(id)) => format!("draft/{id}"),
        _ => "deployment".to_owned(),
    }
}

fn owner_of_log(kind: &str, id: Option<&str>) -> String {
    match (kind, id) {
        ("journey", Some(id)) => format!("journey/{id}"),
        ("route", Some(id)) => format!("route/{id}"),
        _ => "deployment".to_owned(),
    }
}

fn parse(text: &str) -> Result<Value, SqlError> {
    serde_json::from_str(text).map_err(|error| SqlError::Other(format!("stored JSON: {error}")))
}

fn unparse(value: &Value) -> Result<String, SqlError> {
    serde_json::to_string(value).map_err(|error| SqlError::Other(format!("stored JSON: {error}")))
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "passed to `map_err`, which hands over the error"
)]
fn failed(error: cairn_store::StoreError) -> SqlError {
    SqlError::Other(format!("{error:?}"))
}

/// Parses `text`, runs `step` over it with a [`Walk`] in `scope`, and gives back the rewritten
/// text.
fn walk(
    scope: &str,
    typed: &mut Typed,
    keys: &mut BTreeSet<String>,
    text: &str,
    step: &dyn Fn(&mut Walk<'_>, &mut Value),
) -> Result<String, SqlError> {
    let mut value = parse(text)?;
    step(
        &mut Walk {
            owner: scope.to_owned(),
            typed,
            keys,
        },
        &mut value,
    );
    unparse(&value)
}

/// Runs `learn` over the JSON in `text` in `scope`, to know what its decisions are before any
/// condition is read: a condition may name a decision a later mutation or event adds.
fn learned(
    scope: &str,
    typed: &mut Typed,
    text: &str,
    learn: &dyn Fn(&mut Walk<'_>, &Value),
) -> Result<(), SqlError> {
    let value = parse(text)?;
    let mut keys = BTreeSet::new();
    learn(
        &mut Walk {
            owner: scope.to_owned(),
            typed,
            keys: &mut keys,
        },
        &value,
    );
    Ok(())
}

/// Starts `scope` with a copy of `base` the first time it is met: a proposal's candidate, or a
/// log read in order, begins from what its graph holds and changes by its own nodes, never by
/// another scope's.
fn scoped(typed: &mut Typed, scope: &str, base: Option<&BTreeSet<String>>) {
    typed
        .entry(scope.to_owned())
        .or_insert_with(|| base.cloned().unwrap_or_default());
}

/// Rewrites every stored JSON value in place, each read in the scope of what it belongs to:
/// the graph a table row is in, the candidate a proposal draws, the log an event is in.
async fn pass(
    connection: &Connection,
    typed: &mut Typed,
    keys: &mut BTreeSet<String>,
) -> Result<(), SqlError> {
    conditions(connection, typed, keys).await?;
    answers(connection, typed, keys).await?;
    proposals(connection, typed, keys).await?;
    events(connection, typed, keys).await?;
    receipts(connection, typed, keys).await
}

async fn conditions(
    connection: &Connection,
    typed: &mut Typed,
    keys: &mut BTreeSet<String>,
) -> Result<(), SqlError> {
    for row in rows(
        connection,
        "SELECT graph_id, key, relevant_when FROM nodes WHERE relevant_when IS NOT NULL",
        Vec::new(),
    )
    .await?
    {
        let graph = row.text(0).map_err(failed)?;
        let key = row.text(1).map_err(failed)?;
        let condition = row.text(2).map_err(failed)?;
        let new = walk(&graph, typed, keys, &condition, &|walk, value| {
            walk.clause(value);
        })?;
        execute(
            connection,
            "UPDATE nodes SET relevant_when = ?1 WHERE graph_id = ?2 AND key = ?3",
            vec![text(&new), text(&graph), text(&key)],
        )
        .await?;
    }
    Ok(())
}

async fn answers(
    connection: &Connection,
    typed: &mut Typed,
    keys: &mut BTreeSet<String>,
) -> Result<(), SqlError> {
    for row in rows(
        connection,
        "SELECT graph_id, decision, value FROM answers",
        Vec::new(),
    )
    .await?
    {
        let graph = row.text(0).map_err(failed)?;
        let decision = row.text(1).map_err(failed)?;
        let value = row.text(2).map_err(failed)?;
        let new = walk(&graph, typed, keys, &value, &|walk, value| {
            walk.answer(value);
        })?;
        execute(
            connection,
            "UPDATE answers SET value = ?1 WHERE graph_id = ?2 AND decision = ?3",
            vec![text(&new), text(&graph), text(&decision)],
        )
        .await?;
    }
    Ok(())
}

async fn proposals(
    connection: &Connection,
    typed: &mut Typed,
    keys: &mut BTreeSet<String>,
) -> Result<(), SqlError> {
    for row in rows(
        connection,
        "SELECT id, destination_kind, destination_id, mutations, items FROM proposals",
        Vec::new(),
    )
    .await?
    {
        let id = row.text(0).map_err(failed)?;
        let (kind, destination) = (
            row.text(1).map_err(failed)?,
            row.opt_text(2).map_err(failed)?,
        );
        // The candidate: what the destination's graph holds, changed by this proposal's own
        // nodes, all of them known before any condition is read.
        let scope = format!("proposal/{id}");
        let base = typed
            .get(&graph_of_log(&kind, destination.as_deref()))
            .cloned();
        scoped(typed, &scope, base.as_ref());
        learned(
            &scope,
            typed,
            &row.text(3).map_err(failed)?,
            &|walk, value| {
                walk.learn_mutations(value);
            },
        )?;
        let mutations = walk(
            &scope,
            typed,
            keys,
            &row.text(3).map_err(failed)?,
            &|walk, value| walk.mutations(value),
        )?;
        let items = walk(
            &scope,
            typed,
            keys,
            &row.text(4).map_err(failed)?,
            &|walk, value| walk.items(value),
        )?;
        execute(
            connection,
            "UPDATE proposals SET mutations = ?1, items = ?2 WHERE id = ?3",
            vec![text(&mutations), text(&items), text(&id)],
        )
        .await?;
    }
    Ok(())
}

async fn events(
    connection: &Connection,
    typed: &mut Typed,
    keys: &mut BTreeSet<String>,
) -> Result<(), SqlError> {
    let all = rows(
        connection,
        "SELECT seq, patch_id, log_kind, log_id, subject, delta FROM events ORDER BY seq",
        Vec::new(),
    )
    .await?;
    // A log is read in order, from nothing: its decisions are what its own events have made
    // them by then. A patch's events are one change, so a condition may name a decision a later
    // event of the same patch adds: those are known before the patch's first event is read.
    let mut patch: Option<(String, String)> = None;
    for (at, row) in all.iter().enumerate() {
        let seq = row.int(0).map_err(failed)?;
        let (kind, id) = (
            row.text(2).map_err(failed)?,
            row.opt_text(3).map_err(failed)?,
        );
        let scope = format!("log/{}", owner_of_log(&kind, id.as_deref()));
        scoped(typed, &scope, None);
        let this = (row.text(1).map_err(failed)?, scope.clone());
        if patch.as_ref() != Some(&this) {
            for later in &all[at..] {
                let same = later.text(1).map_err(failed)? == this.0
                    && format!(
                        "log/{}",
                        owner_of_log(
                            &later.text(2).map_err(failed)?,
                            later.opt_text(3).map_err(failed)?.as_deref()
                        )
                    ) == scope;
                if !same {
                    break;
                }
                learned(
                    &scope,
                    typed,
                    &later.text(5).map_err(failed)?,
                    &|walk, value| {
                        walk.learn_delta(value);
                    },
                )?;
            }
            patch = Some(this);
        }
        let subject = walk(
            &scope,
            typed,
            keys,
            &row.text(4).map_err(failed)?,
            &|walk, value| walk.subject(value),
        )?;
        let delta = walk(
            &scope,
            typed,
            keys,
            &row.text(5).map_err(failed)?,
            &|walk, value| {
                if let Value::Array(each) = value {
                    for one in each {
                        walk.write(one);
                    }
                }
            },
        )?;
        execute(
            connection,
            "UPDATE events SET subject = ?1, delta = ?2 WHERE seq = ?3",
            vec![text(&subject), text(&delta), crate::sql::int(seq)],
        )
        .await?;
    }
    Ok(())
}

async fn receipts(
    connection: &Connection,
    typed: &mut Typed,
    keys: &mut BTreeSet<String>,
) -> Result<(), SqlError> {
    for row in rows(
        connection,
        "SELECT patch_id, deployment_touched FROM patch_receipts",
        Vec::new(),
    )
    .await?
    {
        let patch = row.text(0).map_err(failed)?;
        let touched = walk(
            "deployment",
            typed,
            keys,
            &row.text(1).map_err(failed)?,
            &|walk, value| {
                if let Value::Array(each) = value {
                    for one in each {
                        walk.record_key(one);
                    }
                }
            },
        )?;
        execute(
            connection,
            "UPDATE patch_receipts SET deployment_touched = ?1 WHERE patch_id = ?2",
            vec![text(&touched), text(&patch)],
        )
        .await?;
    }
    Ok(())
}

/// The keys the relational tables hold.
async fn table_keys(connection: &Connection, keys: &mut BTreeSet<String>) -> Result<(), SqlError> {
    for select in [
        "SELECT key FROM entities",
        "SELECT alias FROM entity_aliases",
        "SELECT entity FROM entity_aliases",
        "SELECT entity FROM entity_emails",
        "SELECT entity FROM answer_entities",
        "SELECT entity FROM role_fill_entities",
        "SELECT entity FROM participation_entities",
    ] {
        for row in rows(connection, select, Vec::new()).await? {
            keys.insert(row.text(0).map_err(failed)?);
        }
    }
    Ok(())
}

/// Runs the step: rewrites every stored JSON value,
/// and records where each old key goes.
pub(crate) async fn run(connection: &Connection) -> Result<(), SqlError> {
    // What each graph holds: the decisions that read identities, exactly, by its own rows.
    let mut typed = Typed::new();
    for row in rows(
        connection,
        "SELECT graph_id, key FROM nodes WHERE answer_type IN ('entity', 'entity_list')",
        Vec::new(),
    )
    .await?
    {
        typed
            .entry(row.text(0).map_err(failed)?)
            .or_default()
            .insert(row.text(1).map_err(failed)?);
    }
    let mut keys = BTreeSet::new();
    pass(connection, &mut typed, &mut keys).await?;
    table_keys(connection, &mut keys).await?;
    let mut seen = BTreeMap::new();
    for old in &keys {
        let new = new_key(old);
        if let Some(other) = seen.insert(new.clone(), old.clone()) {
            return Err(SqlError::Other(format!(
                "identity keys {other} and {old} map to {new}"
            )));
        }
        execute(
            connection,
            "INSERT INTO identity_key_map (old, new) VALUES (?1, ?2)",
            vec![text(old), text(&new)],
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_key_is_opaque_stable_and_distinct() {
        let keys: BTreeSet<String> = ["e_ann", "e_ben", "e_owner"]
            .iter()
            .map(|old| new_key(old))
            .collect();
        assert_eq!(keys.len(), 3);
        for key in &keys {
            let body = key.strip_prefix("who_").unwrap();
            assert_eq!(body.len(), 16);
            assert!(body.bytes().all(|byte| ALPHABET.contains(&byte)));
        }
        assert_eq!(new_key("e_ann"), new_key("e_ann"));
    }
}
