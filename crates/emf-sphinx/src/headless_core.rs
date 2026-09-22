//! Headless core subset (port of C++ `emf-sphinx`).
//!
//! Port target: C++ `emf-sphinx` module of `hebin123456/artop-cpp`.
//!
//! `headless_core` provides the model-processing entry points that libraries
//! such as patcher, validator and report generators build on. It offers a
//! lightweight, self-contained object graph:
//!
//! - [`Root`] — the single model root.
//! - [`Node`] — a named element with attributes and child nodes.
//! - [`Model`] — an index of nodes by path, with O(1) reference resolution.
//! - [`walk`] — depth-first traversal with a visitor that can prune subtrees
//!   and stop early.
//!
//! The design intentionally avoids the full reflection machinery of the
//! sibling EMF crates and can run with zero runtime dependencies.

use std::collections::BTreeMap;

/// A node in the headless object graph.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// Simple name of the node.
    pub name: String,
    /// Element type / classifier, e.g. `EClass`.
    pub kind: String,
    /// Attributes keyed by feature name.
    pub attributes: BTreeMap<String, String>,
    /// Child nodes, in document order.
    pub children: Vec<Node>,
}

impl Node {
    /// Construct a bare node.
    pub fn new(name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: kind.into(),
            attributes: BTreeMap::new(),
            children: Vec::new(),
        }
    }

    /// Set an attribute value.
    pub fn with_attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Push a child node (consuming, for builder chaining).
    pub fn push(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    /// Look up an attribute.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(String::as_str)
    }

    /// Find a direct child by name.
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
}

/// The root of a headless model.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Root {
    /// The root node (often anonymous, e.g. the ARXML root element).
    pub node: Node,
}

impl Root {
    /// Build from a node.
    pub fn new(node: Node) -> Self {
        Self { node }
    }
}

/// An error produced while resolving or processing the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError {
    /// Path or resource that could not be resolved.
    pub target: String,
    /// Reason.
    pub reason: String,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot resolve `{}`: {}", self.target, self.reason)
    }
}

impl std::error::Error for ModelError {}

impl ModelError {
    fn resolve(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            reason: "not found in model".into(),
        }
    }
}

/// A headless model: a root plus the set of every path in the object graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    root: Root,
    /// Set of every absolute path reachable from the root (includes `""`).
    paths: std::collections::BTreeSet<String>,
}

/// Control flow returned by a visitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkControl {
    /// Continue into children.
    Continue,
    /// Skip this node's children but keep walking siblings.
    Prune,
    /// Stop the entire traversal.
    Stop,
}

impl Model {
    /// Index a root into a new model.
    pub fn from_root(root: Root) -> Self {
        let mut paths = std::collections::BTreeSet::new();
        collect_paths(&root.node, "", &mut paths);
        Self { root, paths }
    }

    /// The model root node.
    pub fn root(&self) -> &Node {
        &self.root.node
    }

    /// Resolve a node by `/`-separated absolute path. The empty path is root.
    pub fn resolve(&self, path: &str) -> Option<&Node> {
        let norm = normalize_path(path);
        if !self.paths.contains(&norm) {
            return None;
        }
        if norm.is_empty() {
            return Some(&self.root.node);
        }
        let mut current = &self.root.node;
        for seg in norm.split('/').filter(|s| !s.is_empty()) {
            current = current.child(seg)?;
        }
        Some(current)
    }

    /// Number of indexed nodes.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Resolve and require the node at `path`.
    pub fn require(&self, path: &str) -> Result<&Node, ModelError> {
        self.resolve(path).ok_or_else(|| ModelError::resolve(path))
    }

    /// Walk the model depth-first from `start`, invoking `visitor` on each node.
    pub fn walk_from<F>(&self, start: &Node, visitor: &mut F) -> WalkControl
    where
        F: FnMut(&Node) -> WalkControl,
    {
        match visitor(start) {
            WalkControl::Stop => WalkControl::Stop,
            WalkControl::Prune => WalkControl::Continue,
            WalkControl::Continue => {
                for child in &start.children {
                    if self.walk_from(child, visitor) == WalkControl::Stop {
                        return WalkControl::Stop;
                    }
                }
                WalkControl::Continue
            }
        }
    }
}

/// Record every path reachable from `node` in pre-order.
fn collect_paths(node: &Node, parent: &str, paths: &mut std::collections::BTreeSet<String>) {
    if parent.is_empty() {
        paths.insert(String::new());
    }
    for child in &node.children {
        let path = if parent.is_empty() {
            child.name.to_string()
        } else {
            format!("{parent}/{}", child.name)
        };
        paths.insert(path.clone());
        collect_paths(child, &path, paths);
    }
}

/// Normalize a path: trim and drop leading/trailing slashes.
fn normalize_path(path: &str) -> String {
    path.trim_matches('/').to_string()
}

/// Convenience wrapper: walk the whole model from the root.
pub fn walk<F>(model: &Model, visitor: &mut F) -> WalkControl
where
    F: FnMut(&Node) -> WalkControl,
{
    model.walk_from(model.root(), visitor)
}

/// Count the nodes reachable under `start`.
pub fn count(start: &Node) -> usize {
    fn rec(n: &Node) -> usize {
        1 + n.children.iter().map(rec).sum::<usize>()
    }
    rec(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Node {
        Node::new("Library", "library")
            .with_attr("ns", "http://example/lib")
            .push(
                Node::new("writers", "writers")
                    .push(Node::new("A", "Writer").with_attr("name", "Alice")),
            )
            .push(
                Node::new("books", "books").push(Node::new("B1", "Book").with_attr("title", "T1")),
            )
    }

    #[test]
    fn indexes_and_resolves() {
        let model = Model::from_root(Root::new(sample()));
        assert!(model.resolve("").is_some());
        assert_eq!(
            model.require("writers/A").unwrap().get("name"),
            Some("Alice")
        );
        assert_eq!(model.require("books/B1").unwrap().kind, "Book");
        assert!(model.require("missing").is_err());
    }

    #[test]
    fn walks_depth_first() {
        let model = Model::from_root(Root::new(sample()));
        let mut names = Vec::new();
        let mut v = |n: &Node| {
            names.push(n.name.clone());
            WalkControl::Continue
        };
        walk(&model, &mut v);
        assert_eq!(names, vec!["Library", "writers", "A", "books", "B1"]);
    }

    #[test]
    fn prune_stops_subtree() {
        let model = Model::from_root(Root::new(sample()));
        let mut names = Vec::new();
        let mut v = |n: &Node| {
            names.push(n.name.clone());
            if n.name == "writers" {
                WalkControl::Prune
            } else {
                WalkControl::Continue
            }
        };
        walk(&model, &mut v);
        assert_eq!(names, vec!["Library", "writers", "books", "B1"]);
    }

    #[test]
    fn node_accessors() {
        let n = Node::new("X", "T").with_attr("a", "1");
        assert_eq!(n.get("a"), Some("1"));
        assert_eq!(n.get("none"), None);
    }
}
