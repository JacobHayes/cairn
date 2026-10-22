//! What segment insertion shares with whatever later moves an insertion's keys (B13): the
//! pure function that mints every key an insertion creates from the insertion's key and the
//! key in the segment, and the translation of a node's references from the segment's keys to
//! the graph's. Both are pure, so a proposal's preview and its apply write the same keys.
//!
//! Cost: one pass over a node's fields, each reference one lookup.

use cairn_schema::id::fnv1a_body;
use cairn_schema::{
    AttachmentKey, InsertionKey, KeyRefs, KindKey, MapReferences, Node, NodeField, NodeKey,
    Prefixed, RoleKey,
};

/// B13: the key of type `K` an insertion mints for the object that has `segment_key` in the
/// segment: the prefix and 16 hex digits of the FNV-1a hash of the insertion key, a tag for
/// the object's kind, and the segment key. Two insertions of one version into one graph have
/// different keys, so all their minted keys differ.
///
/// # Panics
///
/// Never: the body is 16 hex digits, a slug within the key length.
#[must_use]
pub(crate) fn mint<K: Prefixed>(insertion: &InsertionKey, segment_key: &str) -> K {
    let tag = K::PREFIX.trim_end_matches('_');
    let body = fnv1a_body(&[
        insertion.as_str().as_bytes(),
        &[0],
        tag.as_bytes(),
        &[0],
        segment_key.as_bytes(),
    ]);
    match K::from_body(&body) {
        Ok(key) => key,
        Err(error) => unreachable!("a minted body is a slug: {error}"),
    }
}

/// The graph's key for each key a segment node refers to.
pub(crate) struct Remap<'a> {
    /// A node key in the segment, to its key in the graph.
    pub node: &'a dyn Fn(&NodeKey) -> NodeKey,
    /// A role key in the segment, to its key in the graph.
    pub role: &'a dyn Fn(&RoleKey) -> RoleKey,
    /// A participation kind key in the segment, to its key in the graph (`owner` to itself).
    pub kind: &'a dyn Fn(&KindKey) -> KindKey,
    /// A resource key in the segment, to its key in the graph.
    pub resource: &'a dyn Fn(&AttachmentKey) -> AttachmentKey,
    /// The first message draft of the node being copied that passes the body limit with its
    /// placeholders translated to longer keys: the segment's resource, and the draft's size.
    pub draft_too_long: Option<(AttachmentKey, usize)>,
}

impl Remap<'_> {
    /// The node as the graph holds it: its own key, parent, edges, and every reference
    /// translated.
    ///
    /// # Errors
    ///
    /// When a message draft, with its placeholders translated to longer keys, passes the body
    /// limit.
    pub(crate) fn copy(&mut self, node: &Node<KeyRefs>) -> Result<Node<KeyRefs>, String> {
        self.draft_too_long = None;
        let copy = node.map_references(self);
        match self.draft_too_long.take() {
            None => Ok(copy),
            Some((resource, bytes)) => Err(format!(
                "message draft {resource} is {bytes} bytes once its keys are minted"
            )),
        }
    }
}

impl MapReferences<KeyRefs, KeyRefs> for Remap<'_> {
    fn node(&mut self, node: &NodeKey, _: Option<NodeField>) -> Option<NodeKey> {
        Some((self.node)(node))
    }

    fn role(&mut self, role: &RoleKey, _: Option<NodeField>) -> Option<RoleKey> {
        Some((self.role)(role))
    }

    fn kind(&mut self, kind: &KindKey) -> Option<KindKey> {
        Some((self.kind)(kind))
    }

    fn own_key(&mut self, key: &NodeKey) -> NodeKey {
        (self.node)(key)
    }

    fn resource_key(&mut self, key: &AttachmentKey) -> AttachmentKey {
        (self.resource)(key)
    }

    fn draft_too_long(&mut self, resource: &AttachmentKey, bytes: usize) {
        self.draft_too_long.get_or_insert((resource.clone(), bytes));
    }
}
