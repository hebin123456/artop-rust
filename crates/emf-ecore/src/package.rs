//! `EPackage`, `EPackageRegistry`, `EFactory`.
//!
//! Port of C++ `emf-ecore/EPackage` and `emf-ecore/EFactory` plus the
//! `EPackageRegistry` global registry.

use crate::{DynamicEObject, EClass, EDataType, EEnum, Val};
use emf_common::uri::Uri;

/// A handle to an owned metamodel package.
pub type PackageRef = std::rc::Rc<std::cell::RefCell<EPackage>>;

/// A container of classifiers (`EPackage`).
///
/// Stores `nsURI`, `nsPrefix`, `name`, its classifiers and a factory handle.
/// The `DynamicEObject` layer uses the package + classifier metadata to answer
/// reflective access.
#[derive(Debug, Clone)]
pub struct EPackage {
    name: String,
    ns_uri: Option<Uri>,
    ns_prefix: String,
    classifiers: Vec<EClass>,
    data_types: Vec<EDataType>,
    enums: Vec<EEnum>,
    /// Child subpackages, keyed by name.
    sub_packages: Vec<PackageRef>,
    /// Whether the package's classifiers were fully registered.
    sealed: bool,
}

impl Default for EPackage {
    fn default() -> Self {
        Self {
            name: String::new(),
            ns_uri: None,
            ns_prefix: String::new(),
            classifiers: Vec::new(),
            data_types: Vec::new(),
            enums: Vec::new(),
            sub_packages: Vec::new(),
            sealed: false,
        }
    }
}

impl EPackage {
    /// New unsealed package.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Package name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Set the name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }
    /// `nsURI` (may be empty).
    pub fn ns_uri(&self) -> Option<&Uri> {
        self.ns_uri.as_ref()
    }
    /// Set `nsURI`.
    pub fn set_ns_uri(&mut self, ns_uri: impl Into<String>) {
        self.ns_uri = Some(Uri::parse(ns_uri));
    }
    /// Clear `nsURI`.
    pub fn clear_ns_uri(&mut self) {
        self.ns_uri = None;
    }
    /// `nsPrefix`.
    pub fn ns_prefix(&self) -> &str {
        &self.ns_prefix
    }
    /// Set `nsPrefix`.
    pub fn set_ns_prefix(&mut self, ns_prefix: impl Into<String>) {
        self.ns_prefix = ns_prefix.into();
    }

    /// Classes owned by this package.
    pub fn classes(&self) -> &[EClass] {
        &self.classifiers
    }
    /// Data types owned by this package.
    pub fn data_types(&self) -> &[EDataType] {
        &self.data_types
    }
    /// Enums owned by this package.
    pub fn enums(&self) -> &[EEnum] {
        &self.enums
    }
    /// Sub packages.
    pub fn sub_packages(&self) -> &[PackageRef] {
        &self.sub_packages
    }

    /// Register a class. Assigns each feature a FeatureID if not already set.
    pub fn add_class(&mut self, mut class: EClass) {
        self.assign_feature_ids(class.e_structural_features_mut());
        self.classifiers.push(class);
    }

    /// Register a data type.
    pub fn add_data_type(&mut self, dt: EDataType) {
        self.data_types.push(dt);
    }

    /// Register an enum.
    pub fn add_enum(&mut self, e: EEnum) {
        self.enums.push(e);
    }

    /// Register a sub package.
    pub fn add_sub_package(&mut self, pkg: PackageRef) {
        let name = pkg.borrow().name().to_string();
        if !self.sub_packages.iter().any(|p| p.borrow().name() == name) {
            self.sub_packages.push(pkg);
        }
    }

    /// A classifier by class name.
    pub fn find_class(&self, name: &str) -> Option<&EClass> {
        self.classifiers.iter().find(|c| c.name() == name)
    }

    /// An enum by name.
    pub fn find_enum(&self, name: &str) -> Option<&EEnum> {
        self.enums.iter().find(|e| e.name() == name)
    }

    /// A data type by name.
    pub fn find_data_type(&self, name: &str) -> Option<&EDataType> {
        self.data_types.iter().find(|d| d.name() == name)
    }

    /// Whether sealed.
    pub fn is_sealed(&self) -> bool {
        self.sealed
    }
    /// Seal the package (freeze class list).
    pub fn seal(&mut self) {
        self.sealed = true;
    }

    /// Assign global FeatureIDs sequentially to features lacking one.
    fn assign_feature_ids(&self, feats: &mut [crate::structural::EStructuralFeature]) {
        let mut next = 0;
        for f in feats.iter_mut() {
            if f.feature_id() < 0 {
                f.set_feature_id(next);
            }
            next += 1;
        }
    }
}

/// A registry of named packages (`EPackageRegistry`), modeled after
/// `EPackage.Registry` in EMF. Bridges from `nsURI` / name to a package and
/// enables reflection lookup for `DynamicEObject`.
#[derive(Debug, Clone, Default)]
pub struct PackageRegistry {
    packages: Vec<PackageRef>,
}

impl PackageRegistry {
    /// New empty registry.
    pub fn new() -> Self {
        Self {
            packages: Vec::new(),
        }
    }

    /// Register a package (by name and by `nsURI`, both are searchable).
    pub fn register(&mut self, pkg: PackageRef) {
        let name = pkg.borrow().name().to_string();
        if !self.packages.iter().any(|p| p.borrow().name() == name) {
            // register by name keyed out of band; keep list for lookup
            let _ = name;
            self.packages.push(pkg);
        }
    }

    /// Look up a package by name.
    pub fn package(&self, name: &str) -> Option<&PackageRef> {
        self.packages.iter().find(|p| p.borrow().name() == name)
    }

    /// Look up a package by `nsURI` (string form).
    pub fn package_by_ns_uri(&self, ns_uri: &str) -> Option<&PackageRef> {
        self.packages
            .iter()
            .find(|p| p.borrow().ns_uri().map(|u| u.to_string()).as_deref() == Some(ns_uri))
    }

    /// All registered packages.
    pub fn packages(&self) -> &[PackageRef] {
        &self.packages
    }

    /// Find a class by name across every package. Returns an owned clone so it
    /// can be used without fighting the `Rc<RefCell>` borrow checker.
    pub fn find_class(&self, cls_name: &str) -> Option<EClass> {
        self.packages
            .iter()
            .find_map(|p| p.borrow().find_class(cls_name).cloned())
    }

    /// Find a package's classes vector by name (borrow-free owned snapshot).
    pub fn classes_of(&self, pkg_name: &str) -> Option<Vec<EClass>> {
        let pkg = self.package(pkg_name)?;
        Some(pkg.borrow().classes().to_vec())
    }
}

impl std::fmt::Display for EPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}<{}>",
            self.name,
            self.ns_uri.as_ref().map(Uri::to_string).unwrap_or_default()
        )
    }
}

/// A factory entry point for creating model objects from an `EClass`
/// (C++ `emf-ecore/EFactory`).
#[derive(Debug, Clone)]
pub struct EFactory {
    package: Option<PackageRef>,
}

impl Default for EFactory {
    fn default() -> Self {
        Self { package: None }
    }
}

impl EFactory {
    /// New factory for a package.
    pub fn new() -> Self {
        Self::default()
    }

    /// The package this factory creates objects for.
    pub fn package(&self) -> Option<&PackageRef> {
        self.package.as_ref()
    }

    /// Bind the factory to a package.
    pub fn set_package(&mut self, pkg: PackageRef) {
        self.package = Some(pkg);
    }

    /// `EFactory.create(EClass)`: instantiate a `DynamicEObject` of the class.
    pub fn create(&self, class: &EClass) -> crate::ObjectRef {
        let obj = DynamicEObject::new(class.clone());
        std::rc::Rc::new(std::cell::RefCell::new(obj))
    }

    /// `EFactory.createFromString(EClassifier, literal)`: parse a literal into
    /// a `Val` for a built-in data type or enum.
    pub fn create_from_string(&self, data_type_name: &str, literal: &str) -> Val {
        crate::datatype::from_string(data_type_name, literal)
    }

    /// `EFactory.convertToString(EClassifier, value)`.
    pub fn convert_to_string(&self, data_type_name: &str, value: &Val) -> String {
        crate::datatype::to_string(data_type_name, value)
    }
}

/// Convenience marker: a remote placeholder for `EPackage::eINSTANCE` bridging
/// classes that don't yet have a full package.
pub fn make_package_ref(pkg: EPackage) -> PackageRef {
    std::rc::Rc::new(std::cell::RefCell::new(pkg))
}
