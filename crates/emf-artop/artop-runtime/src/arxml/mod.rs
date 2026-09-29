//! ARXML (AUTOSAR XML) serialization layer (port of C++
//! `emf-artop/emf-artop-runtime/src/AutosarXMLLoader.cpp` +
//! `AutosarXMLSaver.cpp`).
//!
//! Composed of three pieces, mirroring the C++ file layout:
//!   * [`dom`] — a dependency-free XML DOM that keeps comments and mixed
//!     content, so a faithful arxml round-trip can reproduce the document layout;
//!   * [`store`] — the per-object side tables (mixed content, comments,
//!     reference bookkeeping) shared by the loader and the saver;
//!   * [`loader`] — the arxml deserializer ([`AutosarXMLLoader`]).

pub mod dom;
pub mod loader;
pub mod store;

pub use loader::AutosarXMLLoader;
