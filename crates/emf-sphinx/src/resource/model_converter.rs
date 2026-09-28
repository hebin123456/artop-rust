//! `ModelConverter` interface + `ModelConverterRegistry`.
//!
//! Port of C++ `emf/sphinx/resource/IModelConverter.h` and
//! `ModelConverterRegistry.{h,cpp}` (aligned to Java
//! `org.eclipse.sphinx.emf.resource.IModelConverter` /
//! `ModelConverterRegistry`).

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::resource::{Resource, ResourceHandle};

use crate::metamodel::MetaModelDescriptor;

/// A model converter (e.g. UML -> Ecore).
///
/// C++ returns a raw `Resource*` from `convert`; Rust returns an owned
/// resource handle so the result can outlive the borrow of the source.
pub trait ModelConverter {
    /// The converter id.
    fn id(&self) -> String;
    /// The source meta-model descriptor, if any.
    fn source_meta_model_descriptor(&self) -> Option<Rc<dyn MetaModelDescriptor>>;
    /// The target meta-model descriptor, if any.
    fn target_meta_model_descriptor(&self) -> Option<Rc<dyn MetaModelDescriptor>>;
    /// Convert `source` into a resource for `target_content_type`.
    fn convert(
        &self,
        source: &Resource,
        target_content_type: &str,
    ) -> Option<Box<dyn ResourceHandle>>;
}

thread_local! {
    static CONVERTERS: RefCell<Vec<Rc<dyn ModelConverter>>> = const { RefCell::new(Vec::new()) };
}

/// Tracks all registered [`ModelConverter`]s (aligned to C++
/// `ModelConverterRegistry`, a process-wide singleton; Rust keeps the state in
/// a thread-local so parallel tests stay isolated).
#[derive(Debug, Clone, Copy, Default)]
pub struct ModelConverterRegistry;

impl ModelConverterRegistry {
    /// The singleton registry handle (aligned to C++ `instance()`).
    pub fn instance() -> Self {
        ModelConverterRegistry
    }

    /// Add a converter; duplicates (by handle identity) are ignored.
    pub fn add_converter(&self, converter: Rc<dyn ModelConverter>) {
        CONVERTERS.with(|c| {
            let mut c = c.borrow_mut();
            if !c.iter().any(|existing| Rc::ptr_eq(existing, &converter)) {
                c.push(converter);
            }
        });
    }

    /// Remove a converter by handle identity (`None` is a no-op).
    pub fn remove_converter(&self, converter: Option<&Rc<dyn ModelConverter>>) {
        let Some(converter) = converter else {
            return;
        };
        CONVERTERS.with(|c| {
            let mut c = c.borrow_mut();
            if let Some(i) = c
                .iter()
                .position(|existing| Rc::ptr_eq(existing, converter))
            {
                c.remove(i);
            }
        });
    }

    /// Find the converter whose source/target descriptor identifiers match.
    pub fn find_converter(
        &self,
        source_mm: &str,
        target_mm: &str,
    ) -> Option<Rc<dyn ModelConverter>> {
        CONVERTERS.with(|c| {
            c.borrow().iter().find_map(|conv| {
                let src = conv.source_meta_model_descriptor()?;
                let tgt = conv.target_meta_model_descriptor()?;
                if src.identifier() == source_mm && tgt.identifier() == target_mm {
                    Some(Rc::clone(conv))
                } else {
                    None
                }
            })
        })
    }

    /// All registered converters.
    pub fn all_converters(&self) -> Vec<Rc<dyn ModelConverter>> {
        CONVERTERS.with(|c| c.borrow().clone())
    }
}
