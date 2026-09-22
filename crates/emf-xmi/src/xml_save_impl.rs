//! `XMLSave` / `XMLLoad` injection abstractions for [`XMIResource`].
//!
//! Port of C++ `emf::xmi::XMLSave` / `XMLSaveImpl` (aligned to Java
//! `org.eclipse.emf.ecore.xmi.XMLSave` / `impl.XMLSaveImpl`). `XMLSave` is the
//! abstract `save(resource, output, options)` surface a resource can dispatch
//! its persistence through; `XMLSaveImpl` is the default implementation that
//! produces the real XMI document. The same injection contract exists on the
//! load side via [`XMLLoader`] / [`XMLoaderImpl`].
//!
//! Because Rust has no streams, the save surface returns the serialized text
//! and the load surface consumes text into the resource; the injected mock
//! implementations in the parity suite confirm a custom token is dispatched
//! exactly as in the C++ tests.

use super::xmi_resource::XMIResource;

/// Serialization abstraction (C++ `XMLSave`; Java `XMLSave`).
pub trait XMLSave {
    /// Serialize `resource`'s contents, returning the XMI/XML text.
    fn save(&self, resource: &XMIResource) -> String;
}

/// Default [`XMLSave`] implementation (C++ `XMLSaveImpl`): real XMI output.
#[derive(Debug, Default)]
pub struct XMLSaveImpl;

impl XMLSaveImpl {
    /// New default implementation.
    pub fn new() -> Self {
        Self
    }

    /// A shared handle to a default [`XMLSaveImpl`] as an [`XMLSave`] object.
    pub fn new_boxed() -> std::rc::Rc<dyn XMLSave> {
        std::rc::Rc::new(Self)
    }
}

impl XMLSave for XMLSaveImpl {
    fn save(&self, resource: &XMIResource) -> String {
        resource.save_inner()
    }
}

/// Deserialization abstraction dispatched by [`XMIResource::load_from_string`]
/// (the injection counterpart of C++ `XMLLoad`).
pub trait XMLLoader {
    /// Parse `input` into `resource`, reporting any error.
    fn load(&self, resource: &mut XMIResource, input: &str) -> Result<(), String>;
}

/// Default [`XMLLoader`] implementation: real XMI parse into the resource
/// (mirrors C++ `XMLLoadImpl` delegating to `loadInto()`).
#[derive(Debug, Default)]
pub struct XMLoaderImpl;

impl XMLoaderImpl {
    /// New default implementation.
    pub fn new() -> Self {
        Self
    }
}

impl XMLLoader for XMLoaderImpl {
    fn load(&self, resource: &mut XMIResource, input: &str) -> Result<(), String> {
        resource.load_inner(input)
    }
}