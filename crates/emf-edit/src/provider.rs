//! Item-provider interfaces (port of C++ `emf-edit` `provider/*`, aligned to
//! Java `org.eclipse.emf.edit.provider.*`).
//!
//! Java's item providers adapt model objects into editable items for a viewer:
//! `IItemProvider` is the root interface, with `IItemLabelProvider`,
//! `IStructuredItemContentProvider` and `ITreeItemContentProvider` refining it.
//! They are `Adapter`s, so a provider is attached to a notifier like any other
//! adapter. The C++ units are skeletons; these Rust traits declare the same
//! surface so provider implementations can be built on top.
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use emf_common::notification::Adapter;
use emf_common::value::Val;

/// Root item-provider interface (EMF `IItemProvider`).
pub trait IItemProvider: Adapter {
    /// The children of `object` (EMF `getChildren`).
    fn get_children(&self, object: &Val) -> Vec<Val>;

    /// The parent of `object` (EMF `getParent`).
    fn get_parent(&self, object: &Val) -> Val;
}

/// Provides label text and an image for an object (EMF `IItemLabelProvider`).
pub trait IItemLabelProvider: IItemProvider {
    /// The display text for `object` (EMF `getText`).
    fn get_text(&self, object: &Val) -> String;

    /// The image for `object` (EMF `getImage`); Java returns an `Image`, this
    /// port has no image type and returns `None`.
    fn get_image(&self, object: &Val) -> Option<()>;
}

/// Provides the flat element list for an input (EMF
/// `IStructuredItemContentProvider`).
pub trait IStructuredItemContentProvider: IItemProvider {
    /// The elements for `input` (EMF `getElements`), typically the children.
    fn get_elements(&self, input: &Val) -> Vec<Val>;
}

/// Provides tree navigation for a viewer (EMF `ITreeItemContentProvider`).
pub trait ITreeItemContentProvider: IItemProvider {
    /// Whether `object` has children (EMF `hasChildren`).
    fn has_children(&self, object: &Val) -> bool;
}
