#!/usr/bin/env python3
"""Generate the skeleton stub crates for the artop-rust workspace.
Each crate mirrors one emf-* C++ module; module files map 1:1 to the C++ .cpp
translation units so port progress is traceable.
Both webside generators: only builds/compiles minimal, real API added later.
"""
import os

ROOT = "/workspace/artop-rust"

# crate -> (package name inside, desc, [module names])
CRATES = {
    "artop-ecore-util": (
        "Extra Ecore utility layer (port of C++ `emf-ecore-util`).",
        ["extended_metadata","feature_map","conversion_delegate","copier",
         "e_contents_elist","e_cross_reference_adapter","eobject_containment_elist",
         "eobject_containment_inverse_elist","eobject_elist","eobject_resolving_elist",
         "eobject_validator","eobject_inverse_elist","eobject_inverse_resolving_elist",
         "validator_registry","ecore_adapter_factory","ecore_emap","ecore_switch",
         "ecore_util","ecore_validator","equality_helper","feature_map_util"]),
    "artop-ecore-codegen": (
        "GenModel-driven code generator (port of C++ `emf-ecore-codegen`).",
        ["genmodel","generator","templates"]),
    "artop-xmi": (
        "XMI/XML serialization (port of C++ `emf-xmi`).",
        ["xml_base_handler","sax_mi_handler","sax_xml_handler","xmi_handler",
         "xmi_helper","xmi_loader","xmi_resource","xmi_resource_factory","xmi_saver",
         "xml_handler","xml_helper","xml_load_impl","xml_save_impl"]),
    "artop-xsd": (
        "XSD metamodel (port of C++ `emf-xsd`).",
        ["xsd_metamodel"]),
    "artop-edit": (
        "Editing / Command framework (port of C++ `emf-edit`).",
        ["adapter_factory_editing_domain","add_command","command_helper",
         "composed_adapter_factory","edit_plugin","edit_util","editing_domain",
         "move_command","remove_command","replace_command","set_command",
         "transactional_editing_domain","tree_node"]),
    "artop-compare": (
        "Model comparison (match+diff) (port of C++ `emf-compare`).",
        ["comparison","conflict_detector","diff_engine","diff_filter",
         "equivalence_engine","match_engine","merge_engine","requirement_engine"]),
    "artop-validation": (
        "Model validation, batch + live (port of C++ `emf-validation`).",
        ["annotation_constraint_loader","autosar_constraints","constraint_descriptor",
         "constraint_parser","diagnostician","e_validator","live_validator",
         "validation_service"]),
    "artop-xcore": (
        "Xcore DSL parser (port of C++ `emf-xcore`).",
        ["parser","dsl"]),
    "artop-acceleo": (
        "Acceleo MTL templates / M2T engine (port of C++ `emf-acceleo`).",
        ["mtl_parser","template","m2t_engine"]),
    "artop-sphinx": (
        "Headless core subset (port of C++ `emf-sphinx`).",
        ["headless_core"]),
    "artop-artop-runtime": (
        "AUTOSAR serialization/deserialization, resource/factory/version metadata "
        "(port of C++ `emf-artop/emf-artop-runtime`).",
        ["serialization","deserialization","resource_factory","version_metadata",
         "autosar_metamodel"]),
    "artop-artop-codegen": (
        "Generate static models from .ecore (port of C++ `emf-artop/emf-artop-codegen`).",
        ["generator","ecore_to_model"]),
}

def stub_lib(crate, desc, mods, depends):
    lines = []
    lines.append(f"//! {desc}")
    lines.append(f"//!")
    lines.append(f"//! Port target: C++ `emf-{crate.replace('artop-','')}` module of `hebin123456/artop-cpp`.")
    lines.append(f"//!")
    lines.append(f"//! This file is *skeleton*: each module below is a compile placeholder that")
    lines.append(f"//! will be filled with the port of the corresponding C++ translation unit.")
    lines.append(f"//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.")
    lines.append("")
    for m in mods:
        lines.append(f"pub mod {m} {{")
        lines.append(f"    //! Port target: C++ source unit for `{m}`.")
        lines.append(f"    /// Placeholder marker so the module compiles until the real port lands.")
        lines.append(f"    pub fn api_surface() -> &'static str {{ \"{crate}::{m}\" }}")
        lines.append(f"}}")
        lines.append("")
    lines.append("#[cfg(test)]")
    lines.append("mod tests {")
    lines.append("    #[test]")
    lines.append("    fn skeleton_compiles() {")
    lines.append(f"        assert_eq!(super::{mods[0]}::api_surface(), \"{crate}::{mods[0]}\");")
    lines.append("    }")
    lines.append("}")
    return "\n".join(lines)

def stub_cargo(name, deps):
    deps_txt = "".join(f'{d} = {{ workspace = true }}\n' for d in deps)
    return (
        f"[package]\n"
        f"name = \"{name}\"\n"
        f"version = \"0.1.0\"\n"
        f"edition = \"2021\"\n"
        f"license = \"MIT\"\n"
        f"description = \"See README: artop-rust skeleton crate.\"\n"
        f"\n"
        f"[dependencies]\n"
        f"{deps_txt}"
        f"\n[lints.rust]\n"
    )

def main():
    for crate, (desc, mods) in CRATES.items():
        d = f"{ROOT}/crates/{crate}/src"
        os.makedirs(d, exist_ok=True)
        # dependency: always depend on artop-common; ecore-util additionally on ecore
        deps = ["artop-common"]
        if crate in ("artop-ecore-util",):
            deps.append("artop-ecore")
        with open(f"{ROOT}/crates/{crate}/Cargo.toml", "w") as f:
            f.write(stub_cargo(crate, deps))
        with open(f"{d}/lib.rs", "w") as f:
            f.write(stub_lib(crate, desc, mods, deps))
        print("generated", crate)

    # examples
    for ex in ("arxml-roundtrip", "arxml-validate"):
        d = f"{ROOT}/examples/{ex}/src"
        os.makedirs(d, exist_ok=True)
        with open(f"{ROOT}/examples/{ex}/Cargo.toml", "w") as f:
            f.write(
                f"[package]\nname = \"{ex}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"
                f"publish = false\n\n[dependencies]\nartop-common = {{ workspace = true }}\n")
        name = ex.replace("-", " ")
        with open(f"{d}/main.rs", "w") as f:
            f.write(
                f"//! Example binary `{ex}` (stub). Port of the same-named C++ example.\n"
                f"use artop_common::diagnostic::{{Diagnostic, Severity}};\n\n"
                f"fn main() {{\n"
                f"    let msg = String::from(\"{name}\");\n"
                f"    let d = Diagnostic::new(Severity::Info, \"{ex}\", 0, msg);\n"
                f"    eprintln!(\"{{d}}\");\n"
                f"    assert_ne!(d.severity(), Severity::Error);\n"
                f"}}\n")
        print("generated example", ex)

main()