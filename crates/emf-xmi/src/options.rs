//! XMI serialization options (port of C++ `emf::xmi::XMIOptions`, aligned to
//! Java `org.eclipse.emf.ecore.xmi.XMIResource.XMIResourceOptions`).

/// XMI serialization options.
#[derive(Debug, Clone)]
pub struct XmiOptions {
    /// Whether to declare `xmi:version` on root elements.
    pub declare_xmi: bool,
    /// The XMI version string.
    pub xmi_version: String,
    /// Whether to write `xsi:type` when an object's type differs from the
    /// declared reference type.
    pub declare_xsi_type: bool,
    /// Whether to assign an `xmi:id` to every object.
    pub assign_ids: bool,
    /// Indentation string.
    pub indent: String,
    /// XML encoding used in the `<?xml?>` declaration. Empty follows resource
    /// encoding (default `UTF-8`).
    pub encoding: String,
    /// Whether to emit the `<?xml ?>` declaration.
    pub xml_declaration: bool,
    /// Column at which attribute lines wrap (`0` = no wrap). Kept for future
    /// line-width behavior; the current saver emits one attribute per physical
    /// line when `line_width > 0` and a feature block would exceed it.
    pub line_width: usize,
    /// Force attribute-style output for features that otherwise default to
    /// element style (EMF `OPTION_USE_ENCODED_ATTRIBUTE_STYLE`).
    pub use_encoded_attribute_style: bool,
    /// When true the loader records unmappable features for round-trip.
    pub record_unknown_feature: bool,
}

impl Default for XmiOptions {
    fn default() -> Self {
        Self {
            declare_xmi: true,
            xmi_version: "2.0".to_string(),
            declare_xsi_type: true,
            assign_ids: true,
            indent: "  ".to_string(),
            encoding: String::new(),
            xml_declaration: true,
            line_width: 80,
            use_encoded_attribute_style: false,
            record_unknown_feature: false,
        }
    }
}
