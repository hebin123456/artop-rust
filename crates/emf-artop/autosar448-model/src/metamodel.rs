//! Bridge from the generated static registry to the generic EMF metamodel.
//!
//! The `.ecore`-derived tables in [`crate::registry`] are plain data; this
//! module turns them into an [`emf_ecore::EPackage`] so the *generic* EMF
//! machinery ([`emf_ecore::DynamicEObject`], the XMI loader/saver, and the
//! AUTOSAR arxml layer in `artop-runtime`) can instantiate and reflect AUTOSAR
//! objects exactly like a dynamically-loaded `.ecore` would.
//!
//! Naming follows the AUTOSAR arxml convention: a structural feature is
//! registered under its `xml.name` (e.g. `SHORT-NAME`, `AR-PACKAGE`) rather
//! than the ecore name (`shortName`, `arPackages`). This mirrors the C++
//! `autosar40.ecore`, whose feature *names* are the arxml element names, so an
//! XML element tag can be matched directly against a feature. The original
//! ecore name (and the APRXML serialization metadata) stays available in
//! [`crate::registry::FEATURE_META`] for the codegen / arxml layers.

use std::collections::HashSet;

use emf_ecore::structural::FeatureKind as EcoreFeatureKind;
use emf_ecore::{
    make_package_ref, EAnnotation, EClass, EClassKind, EDataType, EEnum, EPackage,
    EStructuralFeature, PackageRef, PackageRegistry,
};

use crate::registry::{
    DataTypeMeta, EnumMeta, FeatureKind, TypeRef, DATATYPES, ECLASS, ENUMS, FEATURE_META,
};

/// AUTOSAR base namespace URI (`http://autosar.org/schema/r4.0`).
pub const AUTOSAR_BASE_NS_URI: &str = "http://autosar.org/schema/r4.0";
/// Package name of the merged AUTOSAR metamodel (aligns to C++ `autosar40`).
pub const AUTOSAR_PACKAGE_NAME: &str = "autosar40";

/// Map an AUTOSAR primitive data type's `instanceClassName` onto the closest
/// built-in ecore data type, so the generic `datatype::from_string` knows how to
/// parse a literal.
fn ecore_datatype_for(instance_class: &str) -> &'static str {
    match instance_class {
        "java.lang.Boolean" => "EBoolean",
        "java.lang.Integer" => "EInt",
        "java.lang.Long" | "java.math.BigInteger" => "ELong",
        "java.lang.Double" | "java.math.BigDecimal" => "EDouble",
        _ => "EString",
    }
}

/// Resolve a feature's declared type to a name the generic EMF layer can use:
/// the target class / enum name, or a built-in ecore data type for primitives.
fn type_name_of(ty: TypeRef) -> String {
    match ty {
        TypeRef::Class(c) => ECLASS[c as usize].name.to_string(),
        TypeRef::Enum(e) => ENUMS[e as usize].name.to_string(),
        TypeRef::DataType(d) => {
            ecore_datatype_for(DATATYPES[d as usize].instance_class).to_string()
        }
        TypeRef::None => "EString".to_string(),
    }
}

/// The arxml element name a feature is serialized under.
fn xml_feature_name(fm: &crate::registry::FeatureMeta) -> &'static str {
    if fm.xml_name.is_empty() {
        fm.name
    } else {
        fm.xml_name
    }
}

/// Attach the `TaggedValues` annotation carrying the ARXML serialization
/// metadata of a feature (the Rust counterpart of the C++
/// `EAnnotationReader::readFeatureMeta`, which the arxml loader/saver read:
/// `xml.name`, `xml.namePlural`, the APRXML role/type/wrapper flags,
/// `isXmlAttribute` and the sequence offset).
fn tag_feature(f: &mut EStructuralFeature, fm: &crate::registry::FeatureMeta) {
    let mut ann = EAnnotation::new(EStructuralFeature::TAGGED_VALUES);
    if !fm.xml_name.is_empty() {
        ann.set_detail("xml.name", fm.xml_name);
    }
    // The feature is registered under its arxml element name (so the loader can
    // match an XML tag directly), but the reflective constraints look features
    // up by their *ecore* name (C++ `getEStructuralFeature("uuid")`, whose
    // generated model is ecore-named). Record the ecore name as an alias so
    // `DynamicEObject::e_get`/`e_has_feature` resolve both spellings.
    let fname = xml_feature_name(fm);
    if fm.name != fname {
        ann.set_detail("ecore.name", fm.name);
    }
    if !fm.xml_name_plural.is_empty() && fm.xml_name_plural != fm.xml_name {
        ann.set_detail("xml.namePlural", fm.xml_name_plural);
    }
    if !fm.feature_kind.is_empty() {
        ann.set_detail("featureKind", fm.feature_kind);
    }
    if fm.xml_attribute {
        ann.set_detail("isXmlAttribute", "true");
    }
    if !fm.ns_prefix.is_empty() {
        ann.set_detail("xml.nsPrefix", fm.ns_prefix);
    }
    if fm.text_content {
        ann.set_detail("textContent", "true");
    }
    if fm.role_element {
        ann.set_detail("roleElement", "true");
    }
    if fm.role_wrapper {
        ann.set_detail("roleWrapper", "true");
    }
    if fm.type_element {
        ann.set_detail("typeElement", "true");
    }
    if fm.type_wrapper {
        ann.set_detail("typeWrapper", "true");
    }
    if fm.seq_offset != -100 {
        ann.set_detail("internal-xml-sequenceOffset", fm.seq_offset.to_string());
    }
    // `atp.Splitkey` sorts an *unordered* multi-valued feature's children (the
    // C++ `sortChildrenBySplitkey`); `ordered=false` marks a feature as needing
    // that sort (the C++ `isFeatureOrdered`, default true when absent).
    if !fm.splitkey.is_empty() {
        ann.set_detail("atp.Splitkey", fm.splitkey);
    }
    if !fm.ordered {
        ann.set_detail("atp.unordered", "true");
    }
    f.add_annotation(ann);
}

/// Attach the `TaggedValues` annotation carrying a class's arxml element name
/// (`xml.name`) and its `contentKind` (`simple` / `mixed`), so element tags such
/// as `AR-OBJECT` resolve to their `EClass` (mirrors the C++
/// `findEClassByXmlName`) and the loader can pick the simple-content path.
fn tag_class(cls: &mut EClass, cm: &crate::registry::ClassMeta) {
    let xml_differs = !cm.xml_name.is_empty() && cm.xml_name != cm.name;
    if !xml_differs && cm.content_kind.is_empty() {
        return;
    }
    let mut ann = EAnnotation::new(EStructuralFeature::TAGGED_VALUES);
    if xml_differs {
        ann.set_detail("xml.name", cm.xml_name);
        if !cm.xml_name_plural.is_empty() && cm.xml_name_plural != cm.xml_name {
            ann.set_detail("xml.namePlural", cm.xml_name_plural);
        }
    }
    if !cm.content_kind.is_empty() {
        ann.set_detail("contentKind", cm.content_kind);
    }
    cls.add_annotation(ann);
}

/// Build the merged AUTOSAR metamodel as an [`EPackage`].
///
/// Contains every `EClass` (gautosar + autosar448), enum and data type of the
/// static registry. Features are keyed by their arxml element name; class
/// super-types are wired as `eSuperTypes` names so inherited features resolve
/// through the registry.
pub fn build_autosar_package() -> EPackage {
    let mut pkg = EPackage::new(AUTOSAR_PACKAGE_NAME);
    pkg.set_ns_uri(AUTOSAR_BASE_NS_URI);

    for DataTypeMeta {
        name,
        instance_class,
        ..
    } in DATATYPES.iter()
    {
        pkg.add_data_type(EDataType::new(*name, *instance_class));
    }

    for EnumMeta { name, literals, .. } in ENUMS.iter() {
        let mut e = EEnum::new(*name);
        for (i, lit) in literals.iter().enumerate() {
            e.add_literal(*lit, i as i32);
        }
        pkg.add_enum(e);
    }

    for cm in ECLASS.iter() {
        let kind = if cm.abstract_ {
            EClassKind::AbstractClass
        } else {
            EClassKind::Class
        };
        let mut cls = EClass::new(cm.name, kind);
        cls.set_instance_class_name(cm.name);
        cls.set_super_types(
            cm.sups
                .iter()
                .map(|&s| ECLASS[s as usize].name.to_string())
                .collect(),
        );
        tag_class(&mut cls, cm);

        // A class may inherit the same arxml element name twice (e.g. a role
        // element and a plain feature); keep only the first so the generic
        // feature lookups stay unambiguous.
        let mut seen: HashSet<&str> = HashSet::new();
        for &fid in cm.own {
            let fm = &FEATURE_META[fid as usize];
            let fname = xml_feature_name(fm);
            if !seen.insert(fname) {
                continue;
            }
            let ekind = match fm.kind {
                FeatureKind::Attribute => EcoreFeatureKind::Attribute,
                FeatureKind::Reference => EcoreFeatureKind::Reference,
            };
            let mut f = EStructuralFeature::new(fname, ekind, fm.lower, fm.upper);
            f.set_type_name(type_name_of(fm.ty));
            if fm.kind == FeatureKind::Reference {
                f.set_containment(fm.containment);
            }
            f.set_transient(fm.transient);
            f.set_volatile(fm.volatile_);
            f.set_derived(fm.derived);
            tag_feature(&mut f, fm);
            cls.add_feature(f);
        }
        pkg.add_class(cls);
    }

    pkg
}

/// Build the AUTOSAR metamodel package and register it in `registry` under its
/// name, `nsURI` and `nsPrefix` keys.
pub fn register_autosar_metamodel(registry: &mut PackageRegistry) -> PackageRef {
    let pkg = make_package_ref(build_autosar_package());
    registry.register(pkg.clone());
    pkg
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_ecore::{DynamicEObject, Val};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn package_has_base_identity() {
        let pkg = build_autosar_package();
        assert_eq!(pkg.name(), AUTOSAR_PACKAGE_NAME);
        assert_eq!(
            pkg.ns_uri().map(|u| u.to_string()).unwrap(),
            AUTOSAR_BASE_NS_URI
        );
        assert_eq!(pkg.classes().len(), ECLASS.len());
        assert_eq!(pkg.enums().len(), ENUMS.len());
        assert_eq!(pkg.data_types().len(), DATATYPES.len());
    }

    #[test]
    fn autosar_root_class_carries_arxml_features() {
        let mut reg = PackageRegistry::new();
        register_autosar_metamodel(&mut reg);
        let autosar = reg.find_class("AUTOSAR").expect("AUTOSAR class");
        // arxml element names, not ecore names.
        let arpkg = autosar
            .e_all_structural_features(&reg)
            .into_iter()
            .find(|f| f.name() == "AR-PACKAGE")
            .expect("AR-PACKAGE feature");
        assert!(arpkg.is_containment());
        assert_eq!(arpkg.upper_bound(), -1);
        // The AUTOSAR root itself has no SHORT-NAME (arxml `<AUTOSAR>` carries
        // none); `SHORT-NAME` is declared on `Referrable` and reaches concrete
        // classes such as `ARPackage` through `eAllStructuralFeatures`.
        let arpackage = reg.find_class("ARPackage").expect("ARPackage class");
        let sn = arpackage
            .e_all_structural_features(&reg)
            .into_iter()
            .find(|f| f.name() == "SHORT-NAME")
            .expect("SHORT-NAME inherited by ARPackage");
        assert!(!sn.is_reference());
    }

    #[test]
    fn arxml_metadata_annotations_are_attached() {
        let mut reg = PackageRegistry::new();
        register_autosar_metamodel(&mut reg);

        // Every arxml-bearing feature carries the `xml.name` tagged value read
        // back through the annotation surface.
        let autosar = reg.find_class("AUTOSAR").unwrap();
        let arpkg = autosar
            .e_all_structural_features(&reg)
            .into_iter()
            .find(|f| f.name() == "AR-PACKAGE")
            .expect("AR-PACKAGE feature");
        assert_eq!(arpkg.tagged_value("xml.name"), Some("AR-PACKAGE"));

        // Classes whose arxml element differs from the ecore name expose it
        // through `xml_name()`, which powers `find_class_by_xml_name`.
        let arpackage = reg.find_class("ARPackage").unwrap();
        assert_eq!(arpackage.xml_name(), "AR-PACKAGE");
        assert_eq!(
            reg.find_class_by_xml_name("AR-PACKAGE")
                .map(|c| c.name().to_string()),
            Some("ARPackage".to_string())
        );
    }

    #[test]
    fn inheritance_is_wired_by_name() {
        let pkg = build_autosar_package();
        let swc = pkg.find_class("SwcImplementation").unwrap();
        assert!(swc.e_super_types().iter().any(|s| s == "Implementation"));
    }

    #[test]
    fn registered_package_instantiates_and_reflects() {
        let mut reg = PackageRegistry::new();
        let pkg = register_autosar_metamodel(&mut reg);
        assert!(reg.contains_key(AUTOSAR_BASE_NS_URI));
        assert!(reg.contains_key(AUTOSAR_PACKAGE_NAME));
        assert!(Rc::ptr_eq(reg.package(AUTOSAR_PACKAGE_NAME).unwrap(), &pkg));

        // Instantiate <AR-PACKAGE><SHORT-NAME>root</SHORT-NAME></AR-PACKAGE>.
        // `SHORT-NAME` is inherited from `Referrable`, so this exercises the
        // registry-driven `eAllStructuralFeatures` dispatch of `DynamicEObject`.
        let cls = reg.find_class("ARPackage").unwrap();
        let obj = Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg.clone())));
        assert!(obj
            .borrow_mut()
            .e_set_by_name("SHORT-NAME", Val::String("root".into())));
        assert_eq!(
            obj.borrow().e_get_by_name("SHORT-NAME"),
            Some(Val::String("root".into()))
        );
        assert_eq!(obj.borrow().class().name(), "ARPackage");
    }
}
