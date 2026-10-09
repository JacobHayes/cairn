//! Inserting a segment (A21, B13): one mutation copies a published segment version into the
//! patch's journey or route draft under a chosen parent, with every key it creates derived
//! from the insertion's key ([`crate::insertion::mint`]), the segment's roles and kinds
//! mapped onto the graph's, the nodes the insertion leaves out dropped with their subtrees,
//! and the wiring to the graph's own nodes added in the same event. The handler checks what
//! is only knowable at this position (the segment, the version, the keys the mutation names)
//! and writes; every mapping mistake the copy makes (two filling decisions for one role, a
//! dangling reference, a cycle, a duplicate sibling id) is an ordinary violation of the graph
//! the patch produces, found by the one validation pipeline.
//!
//! Cost: one tree of the segment version, O(n log n) for its n nodes, one pass over the
//! host's nodes for the root's siblings and the resource keys it holds, and one write per
//! record the insertion makes.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::limits::NODE_COUNT_MAX;
use cairn_schema::{
    AttachmentKey, Edge, EdgeEnd, Graph, GraphKey, GraphRecord, InsertedEdge, Insertion,
    InsertionKey, KindChoice, KindKey, Limit, LocalEdit, Mutation, Node, NodeKey, NodeState,
    ParticipationKind, Provenance, Role, RoleChoice, RoleKey, Slug, Subject, Title, ViolationCode,
    Write,
};

use super::Session;
use crate::graph::Tree;
use crate::insertion::{Remap, mint};

/// Applies an `insert_segment` mutation.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    let Mutation::InsertSegment {
        insertion,
        segment,
        parent,
        root_id,
        root_title,
        roles,
        kinds,
        omit,
        edges,
    } = mutation
    else {
        unreachable!("{mutation:?} is not an insertion")
    };
    let request = Request {
        insertion,
        segment,
        parent: parent.as_ref(),
        root_id: root_id.as_ref(),
        root_title: root_title.as_ref(),
        roles,
        kinds,
        omit,
        edges,
    };
    match plan(session, &request) {
        Ok(planned) => writes(session, planned),
        Err(problems) => {
            for problem in problems {
                session.reject(problem.code, problem.node.as_ref(), problem.message);
                if let Some(found) = session.violations.last_mut() {
                    if problem.about_insertion {
                        found.at.subject = Some(Subject::Insertion(insertion.clone()));
                    }
                    found.limit = problem.limit;
                }
            }
            Vec::new()
        }
    }
}

/// The mutation's arguments.
struct Request<'a> {
    insertion: &'a InsertionKey,
    segment: &'a cairn_schema::Lineage,
    parent: Option<&'a NodeKey>,
    root_id: Option<&'a Slug>,
    root_title: Option<&'a Title>,
    roles: &'a BTreeMap<RoleKey, RoleChoice>,
    kinds: &'a BTreeMap<KindKey, KindChoice>,
    omit: &'a BTreeSet<NodeKey>,
    edges: &'a [InsertedEdge],
}

/// Something that stops the insertion.
struct Problem {
    code: ViolationCode,
    /// The node of the graph (or the minted key) it is about, which gives it a path.
    node: Option<NodeKey>,
    about_insertion: bool,
    limit: Option<Limit>,
    message: String,
}

impl Problem {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: ViolationCode::InsertionInvalid,
            node: None,
            about_insertion: true,
            limit: None,
            message: message.into(),
        }
    }

    fn at(mut self, node: &NodeKey) -> Self {
        self.node = Some(node.clone());
        self
    }

    /// A key the insertion would mint that the graph holds or retired.
    fn taken(key: &str, what: &str) -> Self {
        Self {
            code: ViolationCode::InsertionKeyTaken,
            node: None,
            about_insertion: true,
            limit: None,
            message: format!(
                "the {what} key {key} the insertion would mint is held or was retired by the graph"
            ),
        }
    }

    fn past(limit: Limit, node: Option<&NodeKey>, message: impl Into<String>) -> Self {
        Self {
            code: ViolationCode::LimitExceeded,
            node: node.cloned(),
            about_insertion: false,
            limit: Some(limit),
            message: message.into(),
        }
    }
}

/// What the insertion writes.
struct Plan {
    roles: Vec<Role<cairn_schema::KeyRefs>>,
    kinds: Vec<ParticipationKind<cairn_schema::KeyRefs>>,
    nodes: Vec<Node<cairn_schema::KeyRefs>>,
    host_edges: Vec<Edge>,
    insertion: Insertion,
}

/// B13 steps 1 to 3 and the wiring: checks the request against the segment version and the
/// graph, and builds what to write, or every problem found.
fn plan(session: &Session<'_>, request: &Request<'_>) -> Result<Plan, Vec<Problem>> {
    let Some(host) = session.graph() else {
        unreachable!("a graph mutation targets an existing graph")
    };
    let segment = admit_segment(session, request)?;
    let tree = Tree::build(&segment.graph);
    let mut problems = Vec::new();
    let root = match tree.roots() {
        [root] => root.clone(),
        roots => {
            return Err(vec![Problem::invalid(format!(
                "the segment version has {} root nodes and an insertion needs exactly one (A21)",
                roots.len()
            ))]);
        }
    };
    let excluded = excluded_nodes(&tree, request.omit);
    check_names(
        host,
        &segment.graph,
        request,
        &root,
        &excluded,
        &mut problems,
    );
    let (roles, added_roles) = map_roles(host, &segment.graph, request, &mut problems);
    let (kinds, added_kinds) = map_kinds(host, &segment.graph, request, &mut problems);
    // A mapping that failed leaves a segment key without a graph key to copy under.
    if !problems.is_empty() {
        return Err(problems);
    }
    let (mut nodes, members) = copy_nodes(
        host,
        &segment.graph,
        request,
        &Copying {
            root: &root,
            excluded: &excluded,
            roles: &roles,
            kinds: &kinds,
        },
        &mut problems,
    );
    let mut host_edges = Vec::new();
    wire(host, request, &mut nodes, &mut host_edges, &mut problems);
    limits(host, &nodes, &added_roles, &added_kinds, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    let insertion = Insertion {
        key: request.insertion.clone(),
        segment: request.segment.clone(),
        parent: request.parent.cloned(),
        roles,
        kinds,
        nodes: members,
    };
    Ok(Plan {
        roles: added_roles,
        kinds: added_kinds,
        nodes: nodes.into_values().collect(),
        host_edges,
        insertion,
    })
}

/// What the copy needs beside the request.
struct Copying<'a> {
    root: &'a NodeKey,
    excluded: &'a BTreeSet<NodeKey>,
    roles: &'a BTreeMap<RoleKey, RoleKey>,
    kinds: &'a BTreeMap<KindKey, KindKey>,
}

/// B13 step 3: every segment node not left out, under its minted key with its references
/// translated; the root takes the parent, id, and title. Also each copy's segment key, and
/// a problem for each key the graph holds or retired.
fn copy_nodes(
    host: &Graph,
    segment: &Graph,
    request: &Request<'_>,
    copying: &Copying<'_>,
    problems: &mut Vec<Problem>,
) -> (
    BTreeMap<NodeKey, Node<cairn_schema::KeyRefs>>,
    BTreeMap<NodeKey, NodeKey>,
) {
    let held_resources = held_resource_keys(host);
    let remap_node = |key: &NodeKey| mint::<NodeKey>(request.insertion, key.as_str());
    let remap_role = |key: &RoleKey| match copying.roles.get(key) {
        Some(mapped) => mapped.clone(),
        None => unreachable!("every role of a valid segment is mapped: {key}"),
    };
    let remap_kind = |key: &KindKey| {
        if *key == KindKey::owner() {
            return KindKey::owner();
        }
        match copying.kinds.get(key) {
            Some(mapped) => mapped.clone(),
            None => unreachable!("every kind of a valid segment is mapped: {key}"),
        }
    };
    let remap_resource =
        |key: &AttachmentKey| mint::<AttachmentKey>(request.insertion, key.as_str());
    let remap = Remap {
        node: &remap_node,
        role: &remap_role,
        kind: &remap_kind,
        resource: &remap_resource,
    };
    let mut nodes = BTreeMap::new();
    let mut members = BTreeMap::new();
    let kept = segment
        .nodes
        .values()
        .filter(|node| !copying.excluded.contains(&node.key));
    for original in kept {
        let mut copy = match remap.node(original) {
            Ok(copy) => copy,
            Err(message) => {
                problems.push(Problem::past(
                    Limit::BodyBytes,
                    Some(&original.key),
                    message,
                ));
                continue;
            }
        };
        if copy.participations.as_map().len() != original.participations.as_map().len() {
            problems.push(Problem::invalid(format!(
                "segment node {} has participations of two kinds that map to one kind of the graph",
                original.key
            )));
        }
        if host.nodes.get(&copy.key).is_some() || host.retired_keys.nodes.contains(&copy.key) {
            problems.push(Problem::taken(copy.key.as_str(), "node"));
        }
        for resource in &copy.resources {
            if held_resources.contains(&resource.key) {
                problems.push(Problem::taken(resource.key.as_str(), "resource"));
            }
        }
        if original.key == *copying.root {
            place_root(host, request, original, &mut copy, problems);
        }
        members.insert(copy.key.clone(), original.key.clone());
        nodes.insert(copy.key.clone(), copy);
    }
    (nodes, members)
}

/// The root's place in the graph: under the parent, with the id and title asked for or the
/// segment root's own (its id suffixed when a sibling has it).
fn place_root(
    host: &Graph,
    request: &Request<'_>,
    original: &Node<cairn_schema::KeyRefs>,
    copy: &mut Node<cairn_schema::KeyRefs>,
    problems: &mut Vec<Problem>,
) {
    copy.parent = request.parent.cloned();
    copy.id = match request.root_id {
        Some(id) => id.clone(),
        None => free_sibling_id(host, request.parent, &original.id).unwrap_or_else(|| {
            problems.push(Problem::invalid(
                "the root's id has no free numeric suffix that fits an id",
            ));
            original.id.clone()
        }),
    };
    if let Some(title) = request.root_title {
        copy.title = title.clone();
    }
}

/// The version the insertion is on, with its route's header checked: a segment, not retired,
/// published, and an insertion key the graph does not hold.
fn admit_segment<'a>(
    session: &'a Session<'_>,
    request: &Request<'_>,
) -> Result<&'a cairn_schema::RouteVersion, Vec<Problem>> {
    let Some(host) = session.graph() else {
        unreachable!("a graph mutation targets an existing graph")
    };
    let route = &request.segment.route;
    let mut problems = Vec::new();
    match session.candidate.routes.get(route) {
        None => problems.push(Problem::invalid(format!("route {route} does not exist"))),
        Some(held) if held.header.kind != cairn_schema::RouteKind::Segment => {
            problems.push(Problem::invalid(format!(
                "route {route} is not a segment: only a segment is inserted (A21)"
            )));
        }
        Some(held) if held.header.retired => {
            problems.push(Problem::invalid(format!(
                "segment {route} is retired and cannot be inserted (A19)"
            )));
        }
        Some(_) => {}
    }
    let version = session.candidate.versions.get(request.segment);
    if version.is_none() {
        problems.push(Problem::invalid(format!(
            "version {} of segment {route} is not published",
            request.segment.version
        )));
    }
    if host.insertions.get(request.insertion).is_some() {
        problems.push(Problem::invalid(format!(
            "the graph holds an insertion with the key {}",
            request.insertion
        )));
    }
    match (version, problems.is_empty()) {
        (Some(version), true) => Ok(version),
        _ => Err(problems),
    }
}

/// Every segment node left out: the omitted ones and everything under them.
fn excluded_nodes(tree: &Tree, omit: &BTreeSet<NodeKey>) -> BTreeSet<NodeKey> {
    let mut excluded = omit.clone();
    for key in omit {
        excluded.extend(tree.descendants(key));
    }
    excluded
}

/// Every key the mutation names exists on its side, and the edges are well formed.
fn check_names(
    host: &Graph,
    segment: &Graph,
    request: &Request<'_>,
    root: &NodeKey,
    excluded: &BTreeSet<NodeKey>,
    problems: &mut Vec<Problem>,
) {
    if let Some(parent) = request.parent
        && host.nodes.get(parent).is_none()
    {
        problems.push(Problem::invalid("the parent is not in the graph").at(parent));
    }
    for (key, choice) in request.roles {
        if segment.roles.get(key).is_none() {
            problems.push(Problem::invalid(format!("the segment has no role {key}")));
        }
        if let RoleChoice::Existing(role) = choice
            && host.roles.get(role).is_none()
        {
            problems.push(Problem::invalid(format!("the graph has no role {role}")));
        }
    }
    for (key, choice) in request.kinds {
        if segment.participation_kinds.get(key).is_none() {
            problems.push(Problem::invalid(format!(
                "the segment has no participation kind {key}"
            )));
        }
        if let KindChoice::Existing(kind) = choice
            && host.participation_kinds.get(kind).is_none()
        {
            problems.push(Problem::invalid(format!(
                "the graph has no participation kind {kind}"
            )));
        }
    }
    for key in request.omit {
        if segment.nodes.get(key).is_none() {
            problems.push(Problem::invalid(format!("the segment has no node {key}")));
        } else if key == root {
            problems.push(Problem::invalid("the segment's root cannot be left out"));
        }
    }
    for edge in request.edges {
        let ends = (&edge.node, &edge.requires);
        let ((EdgeEnd::Segment(inside), EdgeEnd::Host(outside))
        | (EdgeEnd::Host(outside), EdgeEnd::Segment(inside))) = ends
        else {
            problems.push(Problem::invalid(
                "an inserted edge joins one segment node and one node of the graph",
            ));
            continue;
        };
        if segment.nodes.get(inside).is_none() || excluded.contains(inside) {
            problems.push(Problem::invalid(format!(
                "the edge names segment node {inside}, which is not inserted"
            )));
        }
        if host.nodes.get(outside).is_none() {
            problems.push(
                Problem::invalid("the edge names a node that is not in the graph").at(outside),
            );
        }
    }
}

/// `wanted`, or with the smallest numeric suffix not in `taken`; none when no suffix fits an id.
fn free_id(taken: &BTreeSet<Slug>, wanted: &Slug) -> Option<Slug> {
    if !taken.contains(wanted) {
        return Some(wanted.clone());
    }
    // Among `taken.len() + 1` suffixes one is free, unless the id with a suffix is too long.
    (2..).take(taken.len() + 1).find_map(|suffix| {
        let candidate: Slug = format!("{}-{suffix}", wanted.as_str()).parse().ok()?;
        (!taken.contains(&candidate)).then_some(candidate)
    })
}

/// B13 step 2: each segment role mapped onto a graph role: the one named, else the graph's
/// with the same id and cardinality, else a new one minted from the insertion.
fn map_roles(
    host: &Graph,
    segment: &Graph,
    request: &Request<'_>,
    problems: &mut Vec<Problem>,
) -> (BTreeMap<RoleKey, RoleKey>, Vec<Role<cairn_schema::KeyRefs>>) {
    let mut taken: BTreeSet<Slug> = host.roles.values().map(|role| role.id.clone()).collect();
    let mut mapped = BTreeMap::new();
    let mut added = Vec::new();
    for role in segment.roles.values() {
        let existing = match request.roles.get(&role.key) {
            Some(RoleChoice::Existing(key)) => host.roles.get(key).map(Some),
            Some(RoleChoice::Add) => Some(None),
            None => Some(
                host.roles
                    .values()
                    .find(|held| held.id == role.id && held.multi == role.multi),
            ),
        };
        match existing {
            Some(Some(held)) if held.multi != role.multi => {
                problems.push(Problem::invalid(format!(
                    "role {} is {} in the segment and {} in the graph (A6)",
                    role.id,
                    cardinality(role.multi),
                    cardinality(held.multi)
                )));
            }
            Some(Some(held)) => {
                mapped.insert(role.key.clone(), held.key.clone());
            }
            Some(None) => {
                let key = mint::<RoleKey>(request.insertion, role.key.as_str());
                if host.roles.get(&key).is_some() || host.retired_keys.roles.contains(&key) {
                    problems.push(Problem::taken(key.as_str(), "role"));
                }
                let Some(id) = free_id(&taken, &role.id) else {
                    problems.push(Problem::invalid(format!(
                        "role {} has no free numeric suffix that fits an id",
                        role.id
                    )));
                    continue;
                };
                taken.insert(id.clone());
                mapped.insert(role.key.clone(), key.clone());
                added.push(Role {
                    key,
                    id,
                    title: role.title.clone(),
                    multi: role.multi,
                });
            }
            // A named role the graph lacks was reported with the names.
            None => {}
        }
    }
    (mapped, added)
}

/// B13 step 2 for participation kinds, as for roles.
fn map_kinds(
    host: &Graph,
    segment: &Graph,
    request: &Request<'_>,
    problems: &mut Vec<Problem>,
) -> (
    BTreeMap<KindKey, KindKey>,
    Vec<ParticipationKind<cairn_schema::KeyRefs>>,
) {
    let mut taken: BTreeSet<Slug> = host
        .participation_kinds
        .values()
        .map(|kind| kind.id.clone())
        .collect();
    taken.insert(cairn_schema::id::owner_kind_id());
    let mut mapped = BTreeMap::new();
    let mut added = Vec::new();
    for kind in segment.participation_kinds.values() {
        let existing = match request.kinds.get(&kind.key) {
            Some(KindChoice::Existing(key)) => host.participation_kinds.get(key).map(Some),
            Some(KindChoice::Add) => Some(None),
            None => Some(
                host.participation_kinds
                    .values()
                    .find(|held| held.id == kind.id && held.multi == kind.multi),
            ),
        };
        match existing {
            Some(Some(held)) if held.multi != kind.multi => {
                problems.push(Problem::invalid(format!(
                    "participation kind {} is {} in the segment and {} in the graph (A7)",
                    kind.id,
                    cardinality(kind.multi),
                    cardinality(held.multi)
                )));
            }
            Some(Some(held)) => {
                mapped.insert(kind.key.clone(), held.key.clone());
            }
            Some(None) => {
                let key = mint::<KindKey>(request.insertion, kind.key.as_str());
                if host.participation_kinds.get(&key).is_some()
                    || host.retired_keys.kinds.contains(&key)
                {
                    problems.push(Problem::taken(key.as_str(), "participation kind"));
                }
                let Some(id) = free_id(&taken, &kind.id) else {
                    problems.push(Problem::invalid(format!(
                        "participation kind {} has no free numeric suffix that fits an id",
                        kind.id
                    )));
                    continue;
                };
                taken.insert(id.clone());
                mapped.insert(kind.key.clone(), key.clone());
                added.push(ParticipationKind {
                    key,
                    id,
                    title: kind.title.clone(),
                    multi: kind.multi,
                });
            }
            None => {}
        }
    }
    (mapped, added)
}

fn cardinality(multi: bool) -> &'static str {
    if multi {
        "multi valued"
    } else {
        "single valued"
    }
}

/// The resource keys the graph holds on its nodes and in its notes and links.
fn held_resource_keys(host: &Graph) -> BTreeSet<AttachmentKey> {
    let resources = host
        .nodes
        .values()
        .flat_map(|node| node.resources.iter().map(|resource| resource.key.clone()));
    let annotations = host
        .state
        .annotations
        .values()
        .map(|note| note.body.key.clone());
    resources.chain(annotations).collect()
}

/// The root's id among its new siblings: the segment root's own, or the smallest free
/// numeric suffix.
fn free_sibling_id(host: &Graph, parent: Option<&NodeKey>, wanted: &Slug) -> Option<Slug> {
    let siblings: BTreeSet<Slug> = host
        .nodes
        .values()
        .filter(|node| node.parent.as_ref() == parent)
        .map(|node| node.id.clone())
        .collect();
    free_id(&siblings, wanted)
}

/// B13: the wiring. A segment node that requires a graph node takes the edge on its copy;
/// a graph node that requires a segment node gets the edge written on it.
fn wire(
    host: &Graph,
    request: &Request<'_>,
    nodes: &mut BTreeMap<NodeKey, Node<cairn_schema::KeyRefs>>,
    host_edges: &mut Vec<Edge>,
    problems: &mut Vec<Problem>,
) {
    let mut gained: BTreeMap<NodeKey, BTreeSet<NodeKey>> = BTreeMap::new();
    let edges: BTreeSet<&InsertedEdge> = request.edges.iter().collect();
    for edge in edges {
        match (&edge.node, &edge.requires) {
            (EdgeEnd::Segment(inside), EdgeEnd::Host(outside)) => {
                gained
                    .entry(mint::<NodeKey>(request.insertion, inside.as_str()))
                    .or_default()
                    .insert(outside.clone());
            }
            (EdgeEnd::Host(outside), EdgeEnd::Segment(inside)) => {
                host_edges.push(Edge {
                    node: outside.clone(),
                    requires: mint::<NodeKey>(request.insertion, inside.as_str()),
                });
            }
            // Reported with the names.
            _ => {}
        }
    }
    for (minted, outside) in gained {
        let Some(copy) = nodes.get_mut(&minted) else {
            continue;
        };
        let requires: BTreeSet<NodeKey> = copy
            .requires
            .as_set()
            .iter()
            .cloned()
            .chain(outside)
            .collect();
        match cairn_schema::BoundedSet::new(requires) {
            Ok(requires) => copy.requires = requires,
            Err(_) => problems.push(Problem::past(
                Limit::EdgeCountPerNode,
                Some(&minted),
                "past edge_count_per_node_max",
            )),
        }
    }
    let mut counts: BTreeMap<&NodeKey, usize> = BTreeMap::new();
    for edge in host_edges.iter() {
        *counts.entry(&edge.node).or_default() += 1;
    }
    for (node, added) in counts {
        let held = host.nodes.get(node).map_or(0, |found| found.requires.len());
        if Limit::EdgeCountPerNode.check(held + added).is_err() {
            problems.push(Problem::past(
                Limit::EdgeCountPerNode,
                Some(node),
                "past edge_count_per_node_max",
            ));
        }
    }
}

/// The limits the insertion could pass: the node count after it, and the roles and kinds it
/// adds.
fn limits(
    host: &Graph,
    nodes: &BTreeMap<NodeKey, Node<cairn_schema::KeyRefs>>,
    roles: &[Role<cairn_schema::KeyRefs>],
    kinds: &[ParticipationKind<cairn_schema::KeyRefs>],
    problems: &mut Vec<Problem>,
) {
    assert!(host.nodes.len() <= NODE_COUNT_MAX as usize);
    if Limit::NodeCount
        .check(host.nodes.len() + nodes.len())
        .is_err()
    {
        problems.push(Problem::past(
            Limit::NodeCount,
            None,
            "the graph would pass node_count_max with the insertion",
        ));
    }
    if Limit::RoleCount
        .check(host.roles.len() + roles.len())
        .is_err()
    {
        problems.push(Problem::past(
            Limit::RoleCount,
            None,
            "the insertion would add roles past role_count_max",
        ));
    }
    if Limit::KindCount
        .check(host.participation_kinds.len() + kinds.len())
        .is_err()
    {
        problems.push(Problem::past(
            Limit::KindCount,
            None,
            "the insertion would add participation kinds past kind_count_max",
        ));
    }
}

/// B13 steps 4 to 6: the records, in the order replay applies them: roles and kinds, the
/// nodes (and in a journey their initial states), the wiring written on graph nodes with the
/// marker B4 sets on a route-copied one, and the insertion.
fn writes(session: &Session<'_>, planned: Plan) -> Vec<Write> {
    let journey = session.journey().is_some();
    let mut writes = Vec::new();
    writes.extend(
        planned
            .roles
            .into_iter()
            .map(|role| session.put(GraphRecord::Role(role))),
    );
    writes.extend(
        planned
            .kinds
            .into_iter()
            .map(|kind| session.put(GraphRecord::Kind(kind))),
    );
    for node in planned.nodes {
        let state = NodeState::initial(node.kind(), Provenance::FromSegment);
        let key = node.key.clone();
        writes.push(session.put(GraphRecord::Node(node)));
        if journey {
            writes.push(session.put(GraphRecord::NodeState { node: key, state }));
        }
    }
    for edge in planned.host_edges {
        let marker = session.marker(&edge.node, LocalEdit::Requires(edge.requires.clone()));
        writes.push(session.put(GraphRecord::Edge(edge)));
        writes.extend(marker);
    }
    writes.push(session.put(GraphRecord::Insertion(planned.insertion)));
    writes
}

/// B13: each insertion the graph holds narrowed to the members `keeps` is true for; one left
/// with no member goes, in the same mutation.
pub(super) fn narrowed(session: &Session<'_>, keeps: impl Fn(&NodeKey) -> bool) -> Vec<Write> {
    let Some(graph) = session.graph() else {
        unreachable!("a graph patch targets an existing graph")
    };
    let mut writes = Vec::new();
    for insertion in graph.insertions.values() {
        let kept: BTreeMap<NodeKey, NodeKey> = insertion
            .nodes
            .iter()
            .filter(|(member, _)| keeps(member))
            .map(|(member, key)| (member.clone(), key.clone()))
            .collect();
        if kept.len() == insertion.nodes.len() {
            continue;
        }
        writes.push(if kept.is_empty() {
            session.remove(GraphKey::Insertion(insertion.key.clone()))
        } else {
            session.put(GraphRecord::Insertion(Insertion {
                nodes: kept,
                ..insertion.clone()
            }))
        });
    }
    writes
}
