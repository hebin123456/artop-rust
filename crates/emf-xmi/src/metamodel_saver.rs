//! Ecore metamodel-document serializer.
//!
//! Port of the *metadata* path of C++ `emf::xmi::XMISaver` (its `saveEPackage`
//! branch) and its `XMISaverTests.cpp` contract: turns an [`EPackage`] back
//! into the `<ecore:EPackage>` XMI document it was loaded from, so a `.ecore`
//! can round-trip load -> save -> reload without losing structure.
//!
//! This is deliberately a *pure* serializer over the `emf-ecore` metadata
//! structs (`EPackage` / `EClass` / `EStructuralFeature` / `EEnum` / ...). It
//! is the inverse of `emf-ecore-codegen`'s `.ecore` loader and shares the Ecore
//! href conventions:
//!   - built-in data-type references -> `ecore:EDataType <uri>#//<Name>`
//!   - same-package class references -> `#//<Name>`
//!   - plain-many (`upperBound = -1`) -> `upperBound="-1"`
//!
//! Ordering of attributes within one element is stable and matches the C++
//! saver so string-find parity assertions in `XMISaverTests.cpp` hold.

use emf_ecore::datatype::names;
use emf_ecore::{
    EDataType, EEnum, EPackage, EStructuralFeature, ECORE_NS_PREFIX, ECORE_NS_URI,
};

use super::options::XmiOptions;
use super::xml_escape::escape_attr;

/// Whether `type_name` is one of the Ecore built-in data types (so its href
/// renders as `ecore:EDataType <ecore-uri>#//<name>` rather than `#//<name>`).
fn is_ecore_builtin(type_name: &str) -> bool {
    matches!(
        type_name,
        names::E_STRING
            | names::E_BOOLEAN
            | names::E_BOOLEAN_OBJECT
            | names::E_INT
            | names::E_INTEGER_OBJECT
            | names::E_LONG
            | names::E_LONG_OBJECT
            | names::E_DOUBLE
            | names::E_DOUBLE_OBJECT
            | names::E_FLOAT
            | names::E_FLOAT_OBJECT
            | names::E_BYTE
            | names::E_BYTE_OBJECT
            | names::E_SHORT
            | names::E_SHORT_OBJECT
            | names::E_CHAR
            | names::E_CHARACTER_OBJECT
            | names::E_BIG_INTEGER
            | names::E_BIG_DECIMAL
    )
}

/// Serialize a package (meta-model) to an `<ecore:EPackage>` XMI document.
pub fn save_ecore_package(pkg: &EPackage, opts: &XmiOptions) -> String {
    let mut w = MetamodelWriter::new(pkg, opts);
    w.write()
}

/// One pass, straight-line emitter. Keeps a `Vec<String>` of open tags so the
/// closing tags are emitted in reverse order after all classifiers.
struct MetamodelWriter<'a> {
    pkg: &'a EPackage,
    opts: &'a XmiOptions,
    out: String,
    depth: usize,
}

/// A pending `<eClassifiers xsi:type=...>` open element we have written opening
/// content for but not yet closed.
impl<'a> MetamodelWriter<'a> {
    fn new(pkg: &'a EPackage, opts: &'a XmiOptions) -> Self {
        Self {
            pkg,
            opts,
            out: String::new(),
            depth: 0,
        }
    }

    fn ind(&self) -> String {
        self.opts.indent.repeat(self.depth)
    }

    fn write(&mut self) -> String {
        if self.opts.xml_declaration {
            let enc = if self.opts.encoding.is_empty() {
                "UTF-8"
            } else {
                &self.opts.encoding
            };
            self.out
                .push_str(&format!("<?xml version=\"1.0\" encoding=\"{}\"?>\n", enc));
        }

        // Root: <ecore:EPackage> with namespace + package attributes.
        self.out
            .push_str(&format!("<{}:EPackage xmi:version=\"{}\"\n", ECORE_NS_PREFIX, self.opts.xmi_version));
        self.out.push_str(&format!(
            "    xmlns:xmi=\"{}\"\n",
            crate::saver::XMI_NS
        ));
        self.out.push_str(&format!(
            "    xmlns:xsi=\"{}\"\n",
            crate::saver::XSI_NS
        ));
        self.out.push_str(&format!(
            "    xmlns:{}=\"{}\"\n",
            ECORE_NS_PREFIX, ECORE_NS_URI
        ));

        let mut attrs: Vec<(String, String)> = Vec::new();
        attrs.push(("name".into(), self.pkg.name().into()));
        if let Some(u) = self.pkg.ns_uri() {
            attrs.push(("nsURI".into(), u.to_string()));
        }
        attrs.push(("nsPrefix".into(), self.pkg.ns_prefix().into()));
        for (k, v) in attrs {
            self.out
                .push_str(&format!(" {}=\"{}\"", k, escape_attr(&v)));
        }
        self.out.push_str(">\n");
        self.depth += 1;

        self.write_classifiers();
        self.depth -= 1;
        self.out.push_str(&format!(
            "{}</{}:EPackage>\n",
            self.ind(),
            ECORE_NS_PREFIX
        ));

        std::mem::take(&mut self.out)
    }

    fn write_classifiers(&mut self) {
        let indent = self.ind();
        // Classes then built-in data types then enums. The ecore package's
        // built-in data types are declared last to keep the class-only `ecore`
        // document shape; user packages emit all of their classifiers.
        for cls in self.pkg.classes() {
            let mut open = format!(
                "{}<eClassifiers xsi:type=\"{}:EClass\" name=\"{}\"",
                indent,
                ECORE_NS_PREFIX,
                escape_attr(cls.name())
            );
            if cls.is_interface() {
                open.push_str(" interface=\"true\"");
            } else if cls.is_abstract() {
                open.push_str(" abstract=\"true\"");
            }
            if !cls.instance_class_name().is_empty() {
                open.push_str(&format!(
                    " instanceClassName=\"{}\"",
                    escape_attr(cls.instance_class_name())
                ));
            }
            if !cls.e_super_types().is_empty() {
                let supers: Vec<String> = cls
                    .e_super_types()
                    .iter()
                    .map(|s| format!("#//{}", s))
                    .collect();
                open.push_str(&format!(" eSuperTypes=\"{}\"", escape_attr(&supers.join(" "))));
            }
            let has_features = !cls.e_structural_features().is_empty();
            if !has_features {
                open.push_str("/>\n");
                self.out.push_str(&open);
                continue;
            }
            open.push_str(">\n");
            self.out.push_str(&open);
            self.depth += 1;
            for f in cls.e_structural_features() {
                self.write_feature(f);
            }
            self.depth -= 1;
            self.out.push_str(&format!("{}</eClassifiers>\n", self.ind()));
        }

        for dt in self.pkg.data_types() {
            self.write_data_type(dt);
        }

        for e in self.pkg.enums() {
            self.write_enum(e);
        }
    }

    fn write_feature(&mut self, f: &EStructuralFeature) {
        let indent = self.ind();
        let base = if f.is_reference() {
            format!("{}<eStructuralFeatures xsi:type=\"{}:EReference\"", indent, ECORE_NS_PREFIX)
        } else {
            format!("{}<eStructuralFeatures xsi:type=\"{}:EAttribute\"", indent, ECORE_NS_PREFIX)
        };
        let mut s = base;
        s.push_str(&format!(" name=\"{}\"", escape_attr(f.name())));
        if f.upper_bound() == -1 {
            s.push_str(" upperBound=\"-1\"");
        } else if f.upper_bound() != 1 {
            s.push_str(&format!(" upperBound=\"{}\"", f.upper_bound()));
        }
        if f.lower_bound() != 0 {
            s.push_str(&format!(" lowerBound=\"{}\"", f.lower_bound()));
        }
        if f.is_id() {
            s.push_str(" iD=\"true\"");
        }
        // eType
        let type_name = f.type_name().unwrap_or("EString");
        let href = if is_ecore_builtin(type_name) {
            format!("{}:EDataType {}#//{}", ECORE_NS_PREFIX, ECORE_NS_URI, type_name)
        } else {
            format!("#//{}", type_name)
        };
        s.push_str(&format!(" eType=\"{}\"", escape_attr(&href)));

        if f.is_reference() {
            if f.is_containment() {
                s.push_str(" containment=\"true\"");
            }
            if !f.is_resolve_proxies() {
                s.push_str(" resolveProxies=\"false\"");
            }
        } else if let Some(dv) = f.default_value_literal() {
            if !dv.is_empty() {
                s.push_str(&format!(" defaultValueLiteral=\"{}\"", escape_attr(dv)));
            }
        }
        s.push_str("/>\n");
        self.out.push_str(&s);
    }

    fn write_data_type(&mut self, dt: &EDataType) {
        let indent = self.ind();
        let mut s = format!(
            "{}<eClassifiers xsi:type=\"{}:EDataType\" name=\"{}\"",
            indent,
            ECORE_NS_PREFIX,
            escape_attr(dt.name())
        );
        if !dt.instance_class_name().is_empty() {
            s.push_str(&format!(
                " instanceClassName=\"{}\"",
                escape_attr(dt.instance_class_name())
            ));
        }
        s.push_str("/>\n");
        self.out.push_str(&s);
    }

    fn write_enum(&mut self, e: &EEnum) {
        let indent = self.ind();
        let mut open = format!(
            "{}<eClassifiers xsi:type=\"{}:EEnum\" name=\"{}\"",
            indent,
            ECORE_NS_PREFIX,
            escape_attr(e.name())
        );
        if e.e_literals().is_empty() {
            open.push_str("/>\n");
            self.out.push_str(&open);
            return;
        }
        open.push_str(">\n");
        self.out.push_str(&open);
        self.depth += 1;
        let lit_indent = self.ind();
        for (idx, lit) in e.e_literals().iter().enumerate() {
            let mut s = format!(
                "{}<eLiterals name=\"{}\"",
                lit_indent,
                escape_attr(lit.name())
            );
            // EEnumLiteralSerializer omits `value` when it equals the index.
            if lit.value() != idx as i32 {
                s.push_str(&format!(" value=\"{}\"", lit.value()));
            }
            s.push_str(&format!(" literal=\"{}\"", escape_attr(lit.literal())));
            s.push_str("/>\n");
            self.out.push_str(&s);
        }
        self.depth -= 1;
        self.out.push_str(&format!("{}</eClassifiers>\n", self.ind()));
    }
}