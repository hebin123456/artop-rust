//! `.ecore` loader: parse an Ecore metamodel document into [`EPackage`].
//!
//! An `.ecore` file is an XMI document that is itself an *instance* of the
//! Ecore metamodel: root `<ecore:EPackage>`, classifiers as
//! `<eClassifiers xsi:type="ecore:EClass">`, and their features as nested
//! `<eStructuralFeatures xsi:type="ecore:EAttribute|EReference">`. This module
//! walks that tree and materializes the equivalent `emf-ecore` metadata
//! ([`EPackage`] / [`EClass`] / [`EDataType`] / [`EEnum`]) that a downstream
//! code generator (and a package registry) can consume.
//!
//! Port target: the model-loading half of C++ `emf-ecore-codegen`'s
//! `GenModelLoader` — but expressed against the Rust `emf-ecore` metadata API
//! so the result registers cleanly into a [`PackageRegistry`].

use emf_ecore::structural::FeatureKind;
use emf_ecore::{EClass, EClassKind, EDataType, EEnum, EPackage, EStructuralFeature};
use emf_xmi::parser::parse;

/// Parse an `.ecore` document (string) into an owned [`EPackage`].
pub fn load_ecore_package(src: &str) -> Result<EPackage, String> {
    let roots = parse(src)?;
    let root = roots
        .first()
        .ok_or_else(|| "empty .ecore document".to_string())?;
    if root.local != "EPackage" {
        return Err(format!(
            "expected root <ecore:EPackage>, found <{}>",
            root.name
        ));
    }

    let mut pkg = EPackage::new(root.attr("name").unwrap_or("unnamed"));
    if let Some(ns) = root.attr("nsURI") {
        pkg.set_ns_uri(ns);
    }
    if let Some(prefix) = root.attr("nsPrefix") {
        pkg.set_ns_prefix(prefix);
    }

    // Classifiers: children of <eClassifiers>.
    for classifier in root.children.iter().filter(|c| c.local == "eClassifiers") {
        let xsi_type = classifier.attr("xsi:type").unwrap_or("");
        let name = classifier.attr("name").unwrap_or("unnamed");
        match xsi_type {
            t if t.ends_with("EClass") => {
                let is_int = classifier.attr("interface") == Some("true");
                let is_abstract = classifier.attr("abstract") == Some("true");
                let kind = if is_int {
                    EClassKind::Interface
                } else if is_abstract {
                    EClassKind::AbstractClass
                } else {
                    EClassKind::Class
                };
                let mut cls = EClass::new(name, kind);
                if let Some(icn) = classifier.attr("instanceClassName") {
                    cls.set_instance_class_name(icn);
                }
                load_class_features(classifier, &mut cls);
                // Super types (EMF eSuperTypes, usually a comma-separated href list).
                if let Some(supers) = classifier.attr("eSuperTypes") {
                    for s in supers
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                    {
                        cls.add_super_type(href_tail(s)).ok();
                    }
                }
                pkg.add_class(cls);
            }
            t if t.ends_with("EDataType") => {
                let mut dt =
                    EDataType::new(name, classifier.attr("instanceClassName").unwrap_or(""));
                if let Some(lit) = classifier.attr("serializable") {
                    dt.set_serializable(lit == "true");
                }
                pkg.add_data_type(dt);
            }
            t if t.ends_with("EEnum") => {
                let mut e = EEnum::new(name);
                let mut idx = 0;
                for lit in classifier
                    .children
                    .iter()
                    .filter(|c| c.local == "eLiterals")
                {
                    let lname = lit.attr("name").unwrap_or("unnamed");
                    // When `value` is absent the literal takes its ordinal
                    // index (aligns Java EEnumLiteralSerializer, which omits
                    // the auto-incremented value on save).
                    let value = lit
                        .attr("value")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(idx);
                    let literal = e.add_literal(lname, value);
                    if let Some(litstr) = lit.attr("literal") {
                        literal.set_literal(litstr);
                    }
                    idx += 1;
                }
                pkg.add_enum(e);
            }
            _ => {
                return Err(format!(
                    "unsupported classifier xsi:type '{xsi_type}' in <{name}>"
                ));
            }
        }
    }

    Ok(pkg)
}

/// Fill `cls` with structural features parsed from `<eStructuralFeatures>`.
fn load_class_features(classifier: &emf_xmi::parser::XmlNode, cls: &mut EClass) {
    for feat in classifier
        .children
        .iter()
        .filter(|c| c.local == "eStructuralFeatures")
    {
        let xsi_type = feat.attr("xsi:type").unwrap_or("");
        let name = feat.attr("name").unwrap_or("unnamed");
        let et = feat
            .attr("eType")
            .map(href_tail)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "EString".to_string());
        let upper: i32 = feat
            .attr("upperBound")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let lower: i32 = feat
            .attr("lowerBound")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        if xsi_type.ends_with("EReference") {
            let mut f =
                EStructuralFeature::new(name, FeatureKind::Reference, lower, upper);
            f.set_type_name(et.clone());
            if feat.attr("containment") == Some("true") {
                f.set_containment(true);
            }
            if feat.attr("resolveProxies") == Some("false") {
                f.set_resolve_proxies(false);
            }
            cls.add_feature(f);
        } else {
            // EAttribute
            let mut f = EStructuralFeature::new(name, FeatureKind::Attribute, lower, upper);
            f.set_type_name(et.clone());
            if let Some(dv) = feat.attr("defaultValueLiteral") {
                f.set_default_value_literal(dv);
            }
            if feat.attr("iD") == Some("true") {
                f.set_id(true);
            }
            cls.add_feature(f);
        }
    }
}

/// Take the tail of an Ecore href (`//Book` or `...#//Book`) as a plain name.
fn href_tail(href: &str) -> String {
    let after = match href.rfind("#//") {
        Some(i) => &href[i + 3..],
        None => match href.rfind('#') {
            Some(i) => &href[i + 1..],
            None => href,
        },
    };
    // Strip any leading "//" or trailing fragment; match only the last segment.
    let seg = after.split('/').rev().find(|s| !s.is_empty());
    seg.unwrap_or("").to_string()
}

#[cfg(test)]
pub(crate) mod test_utils {
    //! Shared `.ecore` documents used across loading + generation tests.

    /// A small library metamodel (mirrors the C++ `tests/samples/library.ecore`).
    pub const LIBRARY_ECORE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="library"
    nsURI="http://example.com/library/1.0"
    nsPrefix="library">
  <eClassifiers xsi:type="ecore:EClass" name="Library">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name" eType="#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="books" upperBound="-1"
        eType="#//Book" containment="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Book">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="title" eType="#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="pages" eType="#//EInt" defaultValueLiteral="0"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="author" eType="#//Writer" containment="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Writer">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name" eType="#//EString"/>
  </eClassifiers>
</ecore:EPackage>"##;
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_ecore::PackageRegistry;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn loads_package_metadata() {
        let pkg = load_ecore_package(test_utils::LIBRARY_ECORE).unwrap();
        assert_eq!(pkg.name(), "library");
        assert_eq!(pkg.ns_prefix(), "library");
        assert_eq!(
            pkg.ns_uri().unwrap().to_string(),
            "http://example.com/library/1.0"
        );
    }

    #[test]
    fn loads_classes_and_features() {
        let pkg = load_ecore_package(test_utils::LIBRARY_ECORE).unwrap();
        assert_eq!(pkg.classes().len(), 3);
        let lib = pkg.find_class("Library").expect("Library class");
        assert_eq!(lib.name(), "Library");
        let features: Vec<&EStructuralFeature> = lib.e_structural_features().iter().collect();
        assert_eq!(features.len(), 2);
        // order preserved: name (attribute), books (containment many reference)
        assert!(!features[0].is_reference());
        assert_eq!(features[0].type_name().unwrap(), "EString");
        assert!(features[1].is_reference());
        assert!(features[1].is_containment());
        assert!(features[1].is_many());
        assert_eq!(features[1].type_name().unwrap(), "Book");
    }

    #[test]
    fn loads_default_values() {
        let pkg = load_ecore_package(test_utils::LIBRARY_ECORE).unwrap();
        let book = pkg.find_class("Book").unwrap();
        let pages = book
            .e_structural_features()
            .iter()
            .find(|f| f.name() == "pages")
            .unwrap();
        assert_eq!(pages.default_value_literal(), Some("0"));
        assert_eq!(pages.type_name().unwrap(), "EInt");
    }

    #[test]
    fn registers_into_package_registry() {
        let pkg = load_ecore_package(test_utils::LIBRARY_ECORE).unwrap();
        let mut reg = PackageRegistry::new();
        reg.register(Rc::new(RefCell::new(pkg)));
        let found = reg.find_class("Book").expect("Book registered");
        assert_eq!(found.name(), "Book");
        assert_eq!(found.e_structural_features().len(), 3);
    }

    #[test]
    fn rejects_non_ecore_root() {
        let err = load_ecore_package("<foo>bar</foo>").unwrap_err();
        assert!(err.contains("expected root"), "{err}");
    }
}
