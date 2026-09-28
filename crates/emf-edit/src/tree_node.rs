//! `TreeNode` / `TreeIterator` — a generic tree over arbitrary payload
//! (port of C++ `emf-edit` `tree/TreeNode`, aligned to Java
//! `org.eclipse.emf.edit.tree.TreeNode`).
//!
//! Java's `TreeNode` adapts an `EObject` into a `TreeViewer` node with a
//! parent link and ordered children. The C++ port models it with raw pointers;
//! this port uses a [`TreeNodeRef`] handle (an `Rc<RefCell<TreeNode>>`) for
//! children and a `Weak` back-link for the parent, so a child never keeps its
//! parent alive (matching the C++ non-owning `parent_`).
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use emf_common::value::Val;

/// The payload + links of a tree node (EMF `TreeNode`).
pub struct TreeNode {
    data: Val,
    parent: Weak<RefCell<TreeNode>>,
    children: Vec<TreeNodeRef>,
}

/// A shared, mutable tree-node handle (EMF `TreeNode*`).
#[derive(Clone)]
pub struct TreeNodeRef(Rc<RefCell<TreeNode>>);

impl Default for TreeNodeRef {
    fn default() -> Self {
        Self::new(Val::Null)
    }
}

impl TreeNodeRef {
    /// A node carrying `data` (C++ `TreeNode(std::any data)`).
    pub fn new(data: Val) -> Self {
        Self(Rc::new(RefCell::new(TreeNode {
            data,
            parent: Weak::new(),
            children: Vec::new(),
        })))
    }

    /// A node with no payload (C++ `TreeNode()`).
    pub fn empty() -> Self {
        Self::default()
    }

    /// The payload (EMF `getData`).
    pub fn data(&self) -> Val {
        self.0.borrow().data.clone()
    }

    /// Replace the payload (EMF `setData`).
    pub fn set_data(&self, data: Val) {
        self.0.borrow_mut().data = data;
    }

    /// The parent, if any (EMF `getParent`).
    pub fn parent(&self) -> Option<TreeNodeRef> {
        self.0.borrow().parent.upgrade().map(TreeNodeRef)
    }

    /// Set the parent directly, without touching either child list (EMF
    /// `setParent`).
    pub fn set_parent(&self, parent: Option<&TreeNodeRef>) {
        self.0.borrow_mut().parent = match parent {
            Some(p) => Rc::downgrade(&p.0),
            None => Weak::new(),
        };
    }

    /// The ordered children (EMF `getChildren`).
    pub fn children(&self) -> Vec<TreeNodeRef> {
        self.0.borrow().children.clone()
    }

    /// Number of children (EMF `getChildCount`).
    pub fn child_count(&self) -> usize {
        self.0.borrow().children.len()
    }

    /// Append `child`, back-linking its parent (EMF `addChild`).
    pub fn add_child(&self, child: &TreeNodeRef) {
        if self.ptr_eq(child) {
            return;
        }
        child.0.borrow_mut().parent = Rc::downgrade(&self.0);
        self.0.borrow_mut().children.push(child.clone());
    }

    /// Remove `child`, clearing its parent link (EMF `removeChild`).
    pub fn remove_child(&self, child: &TreeNodeRef) {
        let pos = {
            let me = self.0.borrow();
            me.children.iter().position(|c| c.ptr_eq(child))
        };
        if let Some(pos) = pos {
            self.0.borrow_mut().children.remove(pos);
            child.0.borrow_mut().parent = Weak::new();
        }
    }

    /// The child at `index`, if in range (EMF `getChild`).
    pub fn child_at(&self, index: usize) -> Option<TreeNodeRef> {
        self.0.borrow().children.get(index).cloned()
    }

    /// Whether two handles denote the same node (pointer identity).
    pub fn ptr_eq(&self, other: &TreeNodeRef) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A depth-first pre-order iterator over a tree (EMF `TreeIterator`).
pub struct TreeIterator {
    stack: Vec<TreeNodeRef>,
}

impl TreeIterator {
    /// An iterator rooted at `root`.
    pub fn new(root: &TreeNodeRef) -> Self {
        Self {
            stack: vec![root.clone()],
        }
    }

    /// Whether another node remains (C++ `hasNext`).
    pub fn has_next(&self) -> bool {
        !self.stack.is_empty()
    }

    /// The next node in pre-order (C++ `next`).
    pub fn next(&mut self) -> Option<TreeNodeRef> {
        let node = self.stack.pop()?;
        let mut children = node.children();
        // Push children in reverse so the first child is popped first.
        while let Some(child) = children.pop() {
            self.stack.push(child);
        }
        Some(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str) -> TreeNodeRef {
        TreeNodeRef::new(Val::string(name))
    }

    #[test]
    fn add_child_back_links_parent_and_counts() {
        let root = node("root");
        let a = node("a");
        root.add_child(&a);
        assert_eq!(root.child_count(), 1);
        assert!(a.parent().is_some_and(|p| p.ptr_eq(&root)));
    }

    #[test]
    fn remove_child_clears_parent() {
        let root = node("root");
        let a = node("a");
        root.add_child(&a);
        root.remove_child(&a);
        assert_eq!(root.child_count(), 0);
        assert!(a.parent().is_none());
    }

    #[test]
    fn iterator_is_preorder() {
        let root = node("root");
        let a = node("a");
        let b = node("b");
        let a1 = node("a1");
        root.add_child(&a);
        root.add_child(&b);
        a.add_child(&a1);

        let mut it = TreeIterator::new(&root);
        let mut order = Vec::new();
        while it.has_next() {
            if let Some(n) = it.next() {
                order.push(n.data().as_str().unwrap_or_default().to_string());
            }
        }
        assert_eq!(order, vec!["root", "a", "a1", "b"]);
    }
}
