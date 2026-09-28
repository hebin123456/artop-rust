//! Meta-model descriptors: identify the EPackage family a model belongs to.
//!
//! Port of C++ `emf-sphinx/metamodel` (aligned to Java
//! `org.eclipse.sphinx.emf.metamodel`).

pub mod descriptor;
pub mod registry;
pub mod version_data;

pub use descriptor::{AbstractMetaModelDescriptor, MetaModelDescriptor};
pub use registry::MetaModelDescriptorRegistry;
pub use version_data::MetaModelVersionData;
