//! `EMFEditPlugin` — the EMF Edit framework's global plugin singleton (port of
//! C++ `emf-edit` `EMFEditPlugin`, aligned to Java
//! `org.eclipse.emf.edit.EMFEditPlugin`).
//!
//! Java reads the `EMFEditPlugin.properties` resource bundle via the plugin
//! mechanism; the C++ port simplifies this to a static string table plus
//! `initialize()`. This port keeps the same surface: a singleton handle, a
//! string lookup that falls back to the key, and an image lookup that is a
//! placeholder `None`.
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

/// The EMF Edit plugin singleton (EMF `EMFEditPlugin`).
pub struct EMFEditPlugin;

impl EMFEditPlugin {
    /// The process-wide singleton (EMF `EMFEditPlugin.getPlugin`).
    pub fn instance() -> &'static EMFEditPlugin {
        static INSTANCE: EMFEditPlugin = EMFEditPlugin;
        &INSTANCE
    }

    /// Initialize the plugin (C++ `initialize`); loads the string bundle when
    /// one is available.
    pub fn initialize() {
        // No bundled properties yet: behave as an empty bundle.
    }

    /// Shut the plugin down (C++ `shutdown`).
    pub fn shutdown() {}

    /// Translate `key`; with no bundle loaded the key itself is returned (C++
    /// `getString(key, translate=true)`).
    pub fn get_string(&self, key: &str) -> String {
        key.to_string()
    }

    /// Translate `key`, returning `fallback` when the key is unknown (C++
    /// `getString(key, fallback)`).
    pub fn get_string_or<'a>(&self, key: &str, fallback: &'a str) -> &'a str {
        let _ = key;
        fallback
    }

    /// An image descriptor for `key` (C++ `getImage`); Java returns an `Image`,
    /// this port has no image type and returns `None`.
    pub fn get_image(&self, key: &str) -> Option<()> {
        let _ = key;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_is_stable() {
        assert!(std::ptr::eq(
            EMFEditPlugin::instance(),
            EMFEditPlugin::instance()
        ));
    }

    #[test]
    fn unknown_key_falls_back_to_key_or_fallback() {
        let p = EMFEditPlugin::instance();
        assert_eq!(p.get_string("Whatever"), "Whatever");
        assert_eq!(p.get_string_or("Whatever", "fb"), "fb");
        assert!(p.get_image("Whatever").is_none());
    }
}
