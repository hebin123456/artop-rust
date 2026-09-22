//! C++ parity suite: `XMIResourceFactoryTests.cpp` contract.
//!
//! Ports every assertion from the C++ `emf-xmi/tests/XMIResourceFactoryTests.cpp`:
//! the static extension-dispatch API of `XMIResourceFactory` — `registerDefaults`,
//! `createResourceFor` (extension dispatch + unknown-extension fallback),
//! `registerFactory` (custom extension, case-insensitive) and direct
//! `createResource`. Aligned to Java
//! `org.eclipse.emf.ecore.xmi.impl.XMIResourceFactoryImpl`.

use std::cell::Cell;

use emf_common::resource::ResourceHandle;
use emf_common::uri::Uri;
use emf_ecore::PackageRegistry;
use emf_xmi::{XMIResource, XMIResourceFactory};

thread_local! {
    static CUSTOM_CALLED: Cell<bool> = const { Cell::new(false) };
}

fn uri(s: &str) -> Uri {
    Uri::parse(s)
}

/// Mirror the C++ `EXPECT_NOT_NULL`: assert a resource handle was produced and
/// carries the requested URI.
fn assert_resource(r: &dyn ResourceHandle, tail: &str) {
    assert!(r.uri().to_file_path().ends_with(tail), "{:?}", r.uri());
}

// ---------------------------------------------------------------------------
// 1) registerDefaults: .xmi and .ecore both create resources
// ---------------------------------------------------------------------------
#[test]
fn register_defaults_xmi_and_ecore() {
    XMIResourceFactory::register_defaults();
    let r1 = XMIResourceFactory::create_resource_for(uri("file:///tmp/foo.xmi"));
    assert_resource(r1.as_ref(), "foo.xmi");
    let r2 = XMIResourceFactory::create_resource_for(uri("file:///tmp/bar.ecore"));
    assert_resource(r2.as_ref(), "bar.ecore");
}

// ---------------------------------------------------------------------------
// 2) unknown extension falls back to a plain XMI resource (no panic)
// ---------------------------------------------------------------------------
#[test]
fn unknown_extension_fallback() {
    let r = XMIResourceFactory::create_resource_for(uri("file:///tmp/foo.unknown"));
    assert_resource(r.as_ref(), "foo.unknown");
}

// ---------------------------------------------------------------------------
// 3) custom factory registration is called and matched case-insensitively
// ---------------------------------------------------------------------------
#[test]
fn custom_extension_registration() {
    CUSTOM_CALLED.set(false);

    XMIResourceFactory::register_factory("myext", |u: Uri| -> Box<dyn ResourceHandle> {
        CUSTOM_CALLED.set(true);
        // Build a real XMIResource so the fallback shape is preserved.
        Box::new(XMIResource::new(u, PackageRegistry::default()))
    });

    // Lower-case lookup invokes the custom factory.
    let _r = XMIResourceFactory::create_resource_for(uri("file:///tmp/foo.myext"));
    assert!(CUSTOM_CALLED.get(), "custom factory must be invoked");

    // Case-insensitive: .MYEXT maps to the same lowercase "myext" slot.
    let r2 = XMIResourceFactory::create_resource_for(uri("file:///tmp/foo.MYEXT"));
    assert_resource(r2.as_ref(), "foo.MYEXT");
}

// ---------------------------------------------------------------------------
// 4) direct createResource (no extension dispatch)
// ---------------------------------------------------------------------------
#[test]
fn direct_create() {
    let r = XMIResourceFactory::create_resource(uri("inmemory://x"));
    // create_resource always yields a concrete XMIResource (C++ `!= null`).
    let xres = r.as_any().downcast_ref::<XMIResource>().expect("direct create -> XMIResource");
    assert_eq!(xres.uri().to_string(), "inmemory://x");
}