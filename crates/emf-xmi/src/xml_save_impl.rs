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
//!
//! The C++ `save(resource, output, options)` writes into a stream; the Rust
//! side offers the same capability as [`XMLSave::save_to_writer`], whose default
//! body materialises the text and forwards it, so any custom serializer keeps
//! working. A serializer that can emit incrementally (e.g. the artop arxml
//! `AutosarXMLSaver`) overrides it to avoid holding the whole document in
//! memory.

use std::io::{self, Write};

use super::xmi_resource::XMIResource;

/// Serialization abstraction (C++ `XMLSave`; Java `XMLSave`).
pub trait XMLSave {
    /// Serialize `resource`'s contents, returning the XMI/XML text.
    fn save(&self, resource: &XMIResource) -> String;

    /// Stream `resource`'s serialization into `out`.
    ///
    /// The default materialises the document via [`XMLSave::save`] and writes it
    /// in one go, which is correct for any serializer. Implementations that can
    /// write incrementally override this so peak memory stays at the model size
    /// rather than the model plus a full copy of the output text.
    fn save_to_writer(&self, resource: &XMIResource, out: &mut dyn Write) -> io::Result<()> {
        out.write_all(self.save(resource).as_bytes())
    }
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

    /// Parse `input` into `resource`, taking ownership of the document text so
    /// an implementation may release it before building the model. The default
    /// borrows, which is correct for any loader that keeps its own copies of
    /// what it needs — an implementation that can drop the source early
    /// overrides this to actually save the memory.
    fn load_owned(&self, resource: &mut XMIResource, input: String) -> Result<(), String> {
        self.load(resource, input.as_str())
    }
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
