//! `XcoreGenerator` — derive an Ecore `EPackage` from an Xcore AST, and emit a
//! GenModel XML document.
//!
//! Port of C++ `emf-ecore/xcore/XcoreGenerator.cpp` (aligned to Java
//! `org.eclipse.emf.ecore.xcore.XcoreGenerator` +
//! `org.eclipse.emf.codegen.ecore.genmodel.GenModel`).
//!
//! [`XcoreGenerator::generate`] walks the [`PackageDecl`] AST and builds the
//! corresponding `EPackage`/`EClass`/`EAttribute`/`EReference`/`EOperation`/
//! `EEnum`/`EDataType` graph:
//!
//! - `EOperation` gets its return `eType` and `eParameters` (Java parity).
//! - `EReference` ends are cross-linked into `eOpposite` pairs.
//! - Xcore `@Directive` annotations propagate to the derived `EAnnotation`
//!   (using the annotation directive's source URI).
//!
//! [`XcoreGenerator::generate_gen_model`] produces the `.genmodel` XML text.

use crate::dsl::{Annotation, PackageDecl};
use emf_ecore::{
    make_package_ref, EAnnotation, EClass, EClassKind, EDataType, EEnum, EOperation, EPackage,
    EParameter, EStructuralFeature, PackageRef,
};

/// Maps an Xcore primitive type name to its Ecore data type name (C++
/// `XcoreGenerator::resolveClassifier`). Unknown names pass through unchanged.
pub fn ecore_type_name(xcore_type: &str) -> String {
    match xcore_type {
        "String" => "EString",
        "boolean" => "EBoolean",
        "int" => "EInt",
        "long" => "ELong",
        "double" => "EDouble",
        "float" => "EFloat",
        "short" => "EShort",
        "char" => "EChar",
        "byte" => "EByte",
        "Integer" => "EIntegerObject",
        "Long" => "ELongObject",
        "Double" => "EDoubleObject",
        "Float" => "EFloatObject",
        "Short" => "EShortObject",
        "Byte" => "EByteObject",
        "Boolean" => "EBooleanObject",
        "Char" => "ECharacterObject",
        other => other,
    }
    .to_string()
}

/// Propagate one Xcore `@Directive` into a derived [`EAnnotation`], resolving
/// the directive alias to its source URI.
fn to_eannotation(a: &Annotation, xpkg: &PackageDecl) -> EAnnotation {
    let source = xpkg
        .directive_source(&a.directive_name)
        .unwrap_or(a.directive_name.as_str())
        .to_string();
    let mut ea = EAnnotation::new(source);
    for (k, v) in &a.details {
        ea.set_detail(k.clone(), v.clone());
    }
    ea
}

/// Derives Ecore metamodel instances from an Xcore AST (C++ `XcoreGenerator`).
#[derive(Debug, Default, Clone, Copy)]
pub struct XcoreGenerator;

impl XcoreGenerator {
    /// New generator.
    pub fn new() -> Self {
        Self
    }

    /// Derive a fresh `EPackage` from `xpkg` (ownership passes to the caller).
    pub fn generate(&self, xpkg: &PackageDecl) -> PackageRef {
        let mut pkg = EPackage::new(xpkg.name.clone());
        if xpkg.ns_uri.is_empty() {
            pkg.set_ns_uri(format!("http://xcore/{}", xpkg.name));
        } else {
            pkg.set_ns_uri(xpkg.ns_uri.clone());
        }
        if xpkg.ns_prefix.is_empty() {
            pkg.set_ns_prefix(xpkg.name.clone());
        } else {
            pkg.set_ns_prefix(xpkg.ns_prefix.clone());
        }

        let class_names: Vec<&str> = xpkg.classes.iter().map(|c| c.name.as_str()).collect();

        // 1. Build EClass shells with their members (opposites linked below).
        let mut classes: Vec<EClass> = Vec::new();
        for xc in &xpkg.classes {
            let kind = if xc.is_interface {
                EClassKind::Interface
            } else if xc.is_abstract {
                EClassKind::AbstractClass
            } else {
                EClassKind::Class
            };
            let mut cls = EClass::new(xc.name.clone(), kind);
            for st in &xc.super_types {
                if class_names.contains(&st.as_str()) {
                    let _ = cls.add_super_type(st.clone());
                }
            }
            for a in &xc.annotations {
                cls.add_annotation(to_eannotation(a, xpkg));
            }

            for xa in &xc.attributes {
                let mut f = EStructuralFeature::attribute(xa.name.clone());
                f.set_type_name(ecore_type_name(&xa.type_name));
                if xa.multi {
                    f.set_upper_bound(-1);
                }
                f.set_derived(xa.derived);
                f.set_transient(xa.transient);
                f.set_unsettable(xa.unsettable);
                f.set_volatile(xa.volatile);
                f.set_id(xa.id);
                if let Some(d) = &xa.default_value_literal {
                    f.set_default_value_literal(d.clone());
                }
                for a in &xa.annotations {
                    f.add_annotation(to_eannotation(a, xpkg));
                }
                cls.add_feature(f);
            }

            for xr in &xc.references {
                let mut f = EStructuralFeature::reference(xr.name.clone());
                f.set_type_name(xr.type_name.clone());
                f.set_containment(matches!(
                    xr.kind,
                    crate::dsl::ReferenceKind::Containment | crate::dsl::ReferenceKind::Plain
                ));
                if xr.multi {
                    f.set_upper_bound(-1);
                }
                f.set_derived(xr.derived);
                f.set_transient(xr.transient);
                f.set_unsettable(xr.unsettable);
                f.set_volatile(xr.volatile);
                f.set_resolve_proxies(xr.resolve_proxies);
                if let Some(opp) = &xr.opposite_name {
                    f.set_opposite(opp.clone());
                }
                for a in &xr.annotations {
                    f.add_annotation(to_eannotation(a, xpkg));
                }
                cls.add_feature(f);
            }

            for xo in &xc.operations {
                let mut op = EOperation::new(xo.name.clone());
                if !xo.type_name.is_empty() {
                    op.set_return_type(ecore_type_name(&xo.type_name));
                }
                for p in &xo.parameters {
                    let mut ep = EParameter::new(p.name.clone());
                    if !p.type_name.is_empty() {
                        ep.set_type_name(ecore_type_name(&p.type_name));
                    }
                    op.add_eparameter(ep);
                }
                cls.add_operation(op);
            }

            classes.push(cls);
        }

        // 2. Cross-link `eOpposite` pairs (Java `EReference.setEOpposite`).
        for i in 0..classes.len() {
            let declared: Vec<(String, String, String)> = classes[i]
                .e_structural_features()
                .iter()
                .filter_map(|f| {
                    let opp = f.opposite()?;
                    Some((
                        f.name().to_string(),
                        f.type_name().unwrap_or_default().to_string(),
                        opp.to_string(),
                    ))
                })
                .collect();
            for (fname, target, opp) in declared {
                if let Some(ti) = classes.iter().position(|c| c.name() == target) {
                    for f in classes[ti].e_structural_features_mut() {
                        if f.name() == opp {
                            f.set_opposite(fname.clone());
                        }
                    }
                }
            }
        }

        // 3. Register classifiers.
        for cls in classes {
            pkg.add_class(cls);
        }
        for xe in &xpkg.enums {
            let mut e = EEnum::new(xe.name.clone());
            let mut next = 0i32;
            for xl in &xe.literals {
                let v = xl.value.unwrap_or(next);
                e.add_literal(xl.name.clone(), v);
                if let Some(last) = e.e_literals_mut().last_mut() {
                    last.set_literal(if xl.literal.is_empty() {
                        xl.name.clone()
                    } else {
                        xl.literal.clone()
                    });
                }
                next = v + 1;
            }
            pkg.add_enum(e);
        }
        for xd in &xpkg.data_types {
            pkg.add_data_type(EDataType::new(
                xd.name.clone(),
                xd.wrapped_class_name.clone(),
            ));
        }

        make_package_ref(pkg)
    }

    /// Generate GenModel XML text (Java `.genmodel` serialization form), with
    /// the C++ defaults `modelDirectory = "/src"` and `complianceLevel = "8.0"`.
    pub fn generate_gen_model(&self, xpkg: &PackageDecl) -> String {
        self.generate_gen_model_with(xpkg, "/src", "8.0")
    }

    /// Generate GenModel XML text with explicit `modelDirectory` /
    /// `complianceLevel`.
    pub fn generate_gen_model_with(
        &self,
        xpkg: &PackageDecl,
        model_directory: &str,
        compliance_level: &str,
    ) -> String {
        use std::fmt::Write as _;
        let mut os = String::new();

        os.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        let _ = writeln!(
            os,
            "<genmodel:GenModel xmi:version=\"2.0\" xmlns:xmi=\"http://www.omg.org/XMI\" \
             xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
             xmlns:ecore=\"http://www.eclipse.org/emf/2002/Ecore\" \
             xmlns:genmodel=\"http://www.eclipse.org/emf/2002/GenModel\" \
             modelDirectory=\"{model_directory}\" \
             modelPluginID=\"{name}.model\" \
             modelName=\"{name}\" \
             rootExtendsClass=\"org.eclipse.emf.ecore.impl.EObjectImpl\" \
             rootExtendsInterface=\"org.eclipse.emf.ecore.EObject\" \
             rootImplementsInterface=\"\" \
             codeFormatting=\"false\" \
             testsDirectory=\"\" \
             booleanFlagsField=\"\" \
             booleanFlagsReservedBits=\"7\" \
             editPluginClass=\"\" \
             editorPluginClass=\"\" \
             complianceLevel=\"{compliance_level}\" \
             copyrightFields=\"false\" \
             language=\"\">",
            name = xpkg.name,
        );

        let _ = writeln!(os, "  <foreignModel>{}.xcore</foreignModel>", xpkg.name);

        let (base_package, package_name) = match xpkg.name.rsplit_once('.') {
            Some((base, last)) => (base.to_string(), last.to_string()),
            None => (String::new(), xpkg.name.clone()),
        };
        let _ = writeln!(
            os,
            "  <genPackages prefix=\"{package_name}\" \
             disposableProviderFactory=\"true\" \
             ecorePackage=\"{}#/\" \
             basePackage=\"{base_package}\">",
            xpkg.name,
        );

        for xe in &xpkg.enums {
            let _ = writeln!(os, "    <genEnums ecoreEnum=\"{}\">", xe.name);
            for xl in &xe.literals {
                let _ = writeln!(
                    os,
                    "      <genEnumLiterals ecoreEnumLiteral=\"{}\"/>",
                    xl.name
                );
            }
            os.push_str("    </genEnums>\n");
        }

        for xd in &xpkg.data_types {
            let _ = writeln!(os, "    <genDataTypes ecoreDataType=\"{}\"/>", xd.name);
        }

        for xc in &xpkg.classes {
            let _ = writeln!(os, "    <genClasses ecoreClass=\"{}\">", xc.name);
            for xa in &xc.attributes {
                let _ = writeln!(
                    os,
                    "      <genFeatures createChild=\"false\" ecoreFeature=\"ecore:EAttribute {}\"/>",
                    xa.name
                );
            }
            for xr in &xc.references {
                let property = if xr.read_only { "Readonly" } else { "None" };
                let create_child = if matches!(xr.kind, crate::dsl::ReferenceKind::Containment) {
                    "true"
                } else {
                    "false"
                };
                match &xr.opposite_name {
                    Some(opp) => {
                        let _ = writeln!(
                            os,
                            "      <genFeatures property=\"{property}\" notify=\"false\" \
                             createChild=\"{create_child}\" ecoreFeature=\"ecore:EReference {}\" ecoreReverse=\"\">",
                            xr.name
                        );
                        let _ = writeln!(os, "        <genFeature ecoreOpposite=\"{opp}\"/>");
                        os.push_str("      </genFeatures>\n");
                    }
                    None => {
                        let _ = writeln!(
                            os,
                            "      <genFeatures property=\"{property}\" notify=\"false\" \
                             createChild=\"{create_child}\" ecoreFeature=\"ecore:EReference {}\"/>",
                            xr.name
                        );
                    }
                }
            }
            for xo in &xc.operations {
                if xo.parameters.is_empty() {
                    let _ = writeln!(os, "      <genOperations ecoreOperation=\"{}\"/>", xo.name);
                } else {
                    let _ = writeln!(os, "      <genOperations ecoreOperation=\"{}\">", xo.name);
                    for xp in &xo.parameters {
                        let _ = writeln!(
                            os,
                            "        <genParameters ecoreParameter=\"{}\"/>",
                            xp.name
                        );
                    }
                    os.push_str("      </genOperations>\n");
                }
            }
            os.push_str("    </genClasses>\n");
        }

        os.push_str("  </genPackages>\n");
        os.push_str("</genmodel:GenModel>\n");
        os
    }
}
