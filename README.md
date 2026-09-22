# artop-rust

A **Rust** port of [`hebin123456/artop-cpp`](https://github.com/hebin123456/artop-cpp) —
a headless AUTOSAR model tool platform (an EMF + ARTOP reimplementation).
The goal is a 1:1 functional port of every `emf-*` C++ module:

- complete EMF semantics (EObject / EClass / EPackage / Resource / XMI / edit / compare / validate),
- data-driven **reflection** and **generalization** (inheritance) — not Rust subtyping,
- fast compilation even at the full ~2000-class AUTOSAR metamodel scale.

## Porting strategy

Inheritance and reflection are represented as **metadata** (`eSuperTypes` graph),
not as Rust type inheritance. See `crates/artop-metamodel` for the generated
AUTOSAR 4.4.8 registry and its EMF reflection algorithms
(`eAllFeatures`, `isSuperTypeOf`, `eGet`). This keeps a 1925-class metamodel
compiling in seconds. General algorithms dispatch through `&dyn` / enumerated
kinds instead of monomorphizing over every class.

## Workspace layout

Each directory mirrors one C++ module; module names map 1:1 to C++ translation units.

| Crate | Port of (C++) | Status |
|---|---|---|
| `artop-common` | `emf-common` (EObject/EList/Resource/URI/Diagnostic/EPackageRegistry) | skeleton |
| `artop-ecore` | `emf-ecore` (EClass/EPackage/EFactory/ECorePackage) | skeleton |
| `artop-metamodel` | generated model + reflection (`models/autosar448`) | **working** |
| `artop-ecore-util` | `emf-ecore-util` (EcoreUtil/Copier/EMap/validator) | skeleton |
| `artop-ecore-codegen` | `emf-ecore-codegen` (GenModel → codegen) | skeleton |
| `artop-xmi` | `emf-xmi` (XMI/XML serialize, proxies, UUID) | skeleton |
| `artop-xsd` | `emf-xsd` (XSD metamodel) | skeleton |
| `artop-edit` | `emf-edit` (commands / editing domain) | skeleton |
| `artop-compare` | `emf-compare` (match+diff+merge) | skeleton |
| `artop-validation` | `emf-validation` (batch + live) | skeleton |
| `artop-xcore` | `emf-xcore` (Xcore DSL) | skeleton |
| `artop-acceleo` | `emf-acceleo` (MTL/M2T) | skeleton |
| `artop-sphinx` | `emf-sphinx` (headless core) | skeleton |
| `artop-artop-runtime` | `emf-artop-runtime` (AUTOSAR ser/de, versions) | skeleton |
| `artop-artop-codegen` | `emf-artop-codegen` (.ecore → static model) | skeleton |
| `examples/arxml-roundtrip` | `examples/arxml_roundtrip` | skeleton |
| `examples/arxml-validate` | `examples/arxml_validate` | skeleton |

## Build & test (CI does this)

Everything is compiled and tested on GitHub Actions, not locally. To run locally:

```sh
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets
```

## Regenerating the AUTOSAR metamodel

```sh
python3 tools/gen-artop-metamodel.py path/to/autosar448.ecore \
  > crates/artop-metamodel/src/registry.rs
```

## License

MIT (matching `artop-cpp`).

## Module map / port tracking

Each `artop-*` *skeleton* crate contains one placeholder module per C++
translation unit to port. Fill a module to mark that unit ported.