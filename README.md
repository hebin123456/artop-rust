# artop-rust

A **Rust** port of [`hebin123456/artop-cpp`](https://github.com/hebin123456/artop-cpp) —
a headless AUTOSAR model tool platform (an EMF + ARTOP reimplementation).
The goal is a 1:1 functional port of every `emf-*` C++ module:

- complete EMF semantics (EObject / EClass / EPackage / Resource / XMI / edit / compare / validate),
- data-driven **reflection** and **generalization** (inheritance) — not Rust subtyping,
- fast compilation even at the full ~2000-class AUTOSAR metamodel scale.

## Porting strategy

Inheritance and reflection are represented as **metadata** (`eSuperTypes` graph),
not as Rust type inheritance. See `crates/emf-artop/autosar448-model` for the
generated AUTOSAR 4.4.8 registry and its EMF reflection algorithms
(`eAllFeatures`, `isSuperTypeOf`, `eGet`). This keeps a 1925-class metamodel
compiling in seconds. General algorithms dispatch through `&dyn` / enumerated
kinds instead of monomorphizing over every class.

## Naming & workspace layout

The workspace mirrors the C++ repo's module split exactly: the underlying EMF
foundation lives in sibling `emf-*` crates (named `emf-<module>` like the C++
sources in `cpp/emf-cpp/`), while the AUTOSAR-specific ("artop") layers are
grouped together under `crates/emf-artop/` — matching `cpp/emf-cpp/emf-artop/{autosar448-model, artop-runtime, artop-codegen}`.

Each directory mirrors one C++ module; module names map 1:1 to C++ translation units.

| Crate | Port of (C++) | Status |
|---|---|---|
| `emf-common` | `emf-common` (EObject/EList/Resource/URI/Diagnostic/EPackageRegistry) | **working** |
| `emf-ecore` | `emf-ecore` (EClass/EPackage/EFactory/ECorePackage) | **working** |
| `emf-ecore-util` | `emf-ecore-util` (EcoreUtil/Copier/EMap/validator) | skeleton |
| `emf-ecore-codegen` | `emf-ecore-codegen` (GenModel → codegen) | skeleton |
| `emf-xmi` | `emf-xmi` (XMI/XML serialize, proxies, UUID) | skeleton |
| `emf-xsd` | `emf-xsd` (XSD metamodel) | skeleton |
| `emf-edit` | `emf-edit` (commands / editing domain) | skeleton |
| `emf-compare` | `emf-compare` (match+diff+merge) | skeleton |
| `emf-validation` | `emf-validation` (batch + live) | skeleton |
| `emf-xcore` | `emf-xcore` (Xcore DSL) | skeleton |
| `emf-acceleo` | `emf-acceleo` (MTL/M2T) | skeleton |
| `emf-sphinx` | `emf-sphinx` (headless core) | skeleton |
| `emf-artop/autosar448-model` | `emf-artop/autosar448-model` (generated AUTOSAR 4.4.8 registry + reflection) | **working** |
| `emf-artop/artop-runtime` | `emf-artop/emf-artop-runtime` (AUTOSAR ser/de, versions) | skeleton |
| `emf-artop/artop-codegen` | `emf-artop/emf-artop-codegen` (.ecore → static model) | skeleton |
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
python3 tools/gen-autosar448-model.py path/to/autosar448.ecore \
  > crates/emf-artop/autosar448-model/src/registry.rs
```

## License

MIT (matching `artop-cpp`).

## Progress

Detailed, per-module port progress lives in [`docs/PROGRESS.md`](docs/PROGRESS.md).

## Module map / port tracking

Each `artop-*` *skeleton* crate contains one placeholder module per C++
translation unit to port. Fill a module to mark that unit ported.