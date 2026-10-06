//! `DynamicEObject` — a reflective object backed by an `EClass` descriptor.
//!
//! Port of C++ `emf-ecore/DynamicEObject` (aligned to Java
//! `org.eclipse.emf.ecore.impl.DynamicEObjectImpl`). Values are stored in a
//! `HashMap<FeatureID, Val>` plus a set of "set" flags so `eIsSet` and
//! `eUnset` behave like EMF: unsetting restores the class's default value.

use crate::{EClass, Val};
use emf_common::eobject::{EObject, InverseList};
use emf_common::fast_hash::FxHashMap;
use emf_common::notification::{emit, Adapter, EventType, Notification, Notifier, NotifierHandle};
use emf_common::uri::Uri;
use emf_common::value::ObjectRef;
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// A lazily-built index over `eAllStructuralFeatures`.
///
/// The reflective accessors (`e_get` / `e_is_set` / `e_set`) are called many
/// times per object during load and save. Recomputing the inheritance-flattened
/// feature list on every call — which scanned the whole metamodel and cloned
/// every feature — dominated runtime. This cache keeps the resolved list plus a
/// `name -> index` map so each access is a hash probe.
///
/// It is shared by *class* (keyed by [`EClass::instance_id`]) rather than held
/// per object: a feature carries its annotations, so a per-object copy would
/// cost tens of KB each and blow up memory on large documents.
#[derive(Debug, Default)]
struct FeatureCache {
    all: Vec<crate::structural::EStructuralFeature>,
    by_name: FxHashMap<String, usize>,
    /// Indices into `all` of the containment features, in order. Cached so
    /// `eContents` (walked once per object by the loader's index phase and by
    /// the saver) does not re-scan the whole feature list and re-test
    /// `isContainment` for every feature of every object.
    containments: Vec<usize>,
}

thread_local! {
    /// `EClass::instance_id` -> resolved [`FeatureCache`], shared by every
    /// object of that class. `EClass` clones preserve `instance_id`, and
    /// distinct metamodel instances get distinct ids, so this is a stable
    /// class identity for the lifetime of a load/save.
    static CLASS_FEATURE_CACHE: RefCell<HashMap<u64, Rc<FeatureCache>>> =
        RefCell::new(HashMap::new());

    /// `EClass::instance_id` -> the one shared `Rc<EClass>` handed to every
    /// object of that class. A metamodel lookup returns an owned `EClass`
    /// clone, so without this a large document would hold a full `EClass`
    /// (its `name`/`instanceClassName` strings included) per object; keyed by
    /// the clone-stable `instance_id`, all such clones collapse onto a single
    /// allocation. See [`shared_class`].
    static SHARED_CLASSES: RefCell<HashMap<u64, Rc<EClass>>> = RefCell::new(HashMap::new());
}

/// Intern `class` so that every object built from the same metamodel class
/// shares a single `Rc<EClass>`.
///
/// `EClass` clones preserve `instance_id`, so the metamodel lookups that hand
/// the loader an owned `EClass` per object all map to the same id. The extra
/// allocation per object is the whole `EClass` (two `String`s plus the struct);
/// deduplicating it here bounds the model to one class descriptor per class
/// instead of one per object. The cache is thread-local, matching
/// [`CLASS_FEATURE_CACHE`] and the single-threaded `Rc` model.
fn shared_class(class: EClass) -> Rc<EClass> {
    let id = class.instance_id();
    SHARED_CLASSES.with(|m| {
        let mut m = m.borrow_mut();
        if let Some(rc) = m.get(&id) {
            return rc.clone();
        }
        let rc = Rc::new(class);
        m.insert(id, rc.clone());
        rc
    })
}

/// A back-link from a contained object to its container: a weak handle to the
/// parent plus the name of the containment feature that owns this object.
/// Weak avoids reference cycles between parent and child.
pub type ContainerBackref = (Weak<RefCell<dyn EObject>>, String);

/// Registry of reverse-reference lists keyed by feature id (C++
/// `impl::BasicEObject::eInverseELists_`). A list registers itself on the owner
/// object; the registry holds shared handles and compares them by identity on
/// unregistration, mirroring the C++ raw `EInverseList*` comparison.
#[derive(Default)]
struct InverseLists {
    lists: HashMap<i32, Rc<RefCell<dyn InverseList>>>,
}

impl InverseLists {
    fn register(&mut self, feature_id: i32, list: Rc<RefCell<dyn InverseList>>) {
        self.lists.insert(feature_id, list);
    }

    fn unregister(&mut self, feature_id: i32, list: &Rc<RefCell<dyn InverseList>>) {
        // Only drop the entry when it *is* the list being unregistered; a
        // mismatched list is a no-op (C++ `it->second == list`).
        if let Some(registered) = self.lists.get(&feature_id) {
            if Rc::ptr_eq(registered, list) {
                self.lists.remove(&feature_id);
            }
        }
    }

    fn get(&self, feature_id: i32) -> Option<Rc<RefCell<dyn InverseList>>> {
        self.lists.get(&feature_id).cloned()
    }
}

impl std::fmt::Debug for InverseLists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InverseLists")
            .field("len", &self.lists.len())
            .finish()
    }
}

/// Per-object storage for the explicitly-set features, keyed by the feature's
/// index in the class's flattened feature list (see [`FeatureCache::all`]).
///
/// Keyed by index, `FxHashMap<u32, Val>` was the obvious store, but a hash table
/// pays a large fixed cost that is pure overhead for the common case of a leaf
/// object that sets one or two features: the smallest table is a control group
/// plus four 40-byte buckets (~192 B) where the payload itself is 40 B, and an
/// *empty* `RawTable` header still occupies 32 B inside every object. A sorted,
/// exactly-sized box pays only for what is stored — one 40-byte block for one
/// set feature, nothing at all for none — and the field shrinks from a 32-byte
/// map header to a 16-byte fat pointer. Entries are few, so a lookup is a binary
/// search and an insert shifts a handful of slots; both are cheaper than hashing
/// at this size, and the hot paths (`e_get`/`e_is_set`/save) stay within a few
/// comparisons.
#[derive(Debug, Default)]
struct FeatureMap {
    /// `(feature index, value)` pairs sorted ascending by index; `None` = unset.
    entries: Option<Box<[(u32, Val)]>>,
}

impl FeatureMap {
    /// The value stored for `key`, if set.
    fn get(&self, key: u32) -> Option<&Val> {
        let entries = self.entries.as_deref()?;
        let i = entries.binary_search_by_key(&key, |e| e.0).ok()?;
        Some(&entries[i].1)
    }

    /// Mutable access to the value stored for `key`, if set.
    fn get_mut(&mut self, key: u32) -> Option<&mut Val> {
        let entries = self.entries.as_deref_mut()?;
        let i = entries.binary_search_by_key(&key, |e| e.0).ok()?;
        Some(&mut entries[i].1)
    }

    /// Store `value` under `key`, returning the previous value if there was one.
    fn insert(&mut self, key: u32, value: Val) -> Option<Val> {
        if self.entries.is_none() {
            let boxed: Box<[(u32, Val)]> = Box::new([(key, value)]);
            self.entries = Some(boxed);
            return None;
        }
        match self
            .entries
            .as_deref()
            .expect("entries present")
            .binary_search_by_key(&key, |e| e.0)
        {
            Ok(i) => Some(std::mem::replace(
                &mut self.entries.as_deref_mut().expect("entries present")[i].1,
                value,
            )),
            Err(pos) => {
                // Rebuild one larger, *moving* the existing entries — a rebuild
                // is inherent to an exactly-sized box, and no `Val` is cloned.
                let old = Vec::from(self.entries.take().expect("entries present"));
                let mut next: Vec<(u32, Val)> = Vec::with_capacity(old.len() + 1);
                let mut it = old.into_iter();
                for _ in 0..pos {
                    next.push(it.next().expect("entry before insertion point"));
                }
                next.push((key, value));
                next.extend(it);
                // `next` is filled to capacity, so this is a single allocation.
                self.entries = Some(next.into_boxed_slice());
                None
            }
        }
    }

    /// Remove and return the value stored for `key`, if any.
    fn remove(&mut self, key: u32) -> Option<Val> {
        let i = self
            .entries
            .as_deref()?
            .binary_search_by_key(&key, |e| e.0)
            .ok()?;
        let mut v = Vec::from(self.entries.take().expect("entries present"));
        let (_, removed) = v.remove(i);
        self.entries = if v.is_empty() {
            None
        } else {
            v.shrink_to_fit();
            Some(v.into_boxed_slice())
        };
        Some(removed)
    }

    /// The set entries as `(feature index, value)` pairs, in index order.
    fn iter(&self) -> impl Iterator<Item = (u32, &Val)> {
        let entries: &[(u32, Val)] = self.entries.as_deref().unwrap_or(&[]);
        entries.iter().map(|e| (e.0, &e.1))
    }
}

#[derive(Debug)]
pub struct DynamicEObject {
    /// The class descriptor.
    ///
    /// Shared through `Rc` rather than owned: a metamodel lookup yields an
    /// owned `EClass` clone per object, which on a large document means one
    /// full class descriptor (its `name`/`instanceClassName` strings included)
    /// per object. [`shared_class`] collapses every clone of the same class
    /// onto one allocation, so the descriptor cost is per class, not per
    /// object. `EClass` is internally immutable-after-build (its vectors are
    /// `Rc`+copy-on-write), so sharing is observationally transparent.
    pub e_class: Rc<EClass>,
    /// Multi-purpose storage keyed by the feature's index in [`FeatureCache::all`].
    ///
    /// Keyed by the per-class feature *index* rather than by name. A `String`
    /// key cost one heap allocation plus a 24-byte key slot per set feature,
    /// which dominated the model's memory for a document with tens of thousands
    /// of objects. The index is stable and unambiguous within an object: an
    /// object has exactly one class, and `by_name` — which maps both the arxml
    /// register name and the `ecore.name` alias to a single index — resolves any
    /// spelling to it, so this does not reintroduce the cross-package feature-id
    /// collision that ruled out raw feature ids (those are only unique within an
    /// EPackage; an index is unique within the class's flattened feature list).
    ///
    /// Presence in this map *is* the EMF "is set" state: a feature whose value
    /// was written — even when equal to its class default — is stored here, and
    /// reading it back yields that value. This replaces a second
    /// `HashSet<String>` of set flags, which duplicated every key (an extra
    /// allocation and hash lookup per set/query) for no extra information.
    ///
    /// Stored in a [`FeatureMap`] rather than a `HashMap`: see that type for why
    /// an exactly-sized sorted box beats a hash table at this entry count.
    dynamic_settings: FeatureMap,
    /// The container (weak parent + containment feature name), if this object is
    /// owned by another object through a containment reference.
    container: Option<ContainerBackref>,
    /// Reverse-reference lists registered by feature id (C++ `BasicEObject`).
    inverse_lists: InverseLists,
    /// Registry snapshot used to resolve the inheritance graph.
    registry: Option<crate::package::PackageRegistry>,
    /// A proxy URI, when this object stands in for an unresolved reference
    /// (`e_is_proxy` == true). `None` for a concrete object.
    proxy_uri: Option<Uri>,
    /// The notification sink for this object (EMF `Notifier`). Adapters
    /// attached here receive SET/UNSET notifications when features change.
    ///
    /// Lazily materialized: the overwhelming majority of objects in a large
    /// document never get an adapter, so eagerly allocating a
    /// `Rc<RefCell<Notifier>>` (an `Rc` header plus a `Vec` + flag — ~56 bytes)
    /// per object is pure waste. `OnceCell` keeps the field at one pointer and
    /// only pays for the sink when [`Self::notifier`] / [`Self::add_adapter`]
    /// is reached; the reflective hot paths gate on
    /// [`Self::e_notification_required`], which stays allocation-free while the
    /// cell is empty.
    notifier: OnceCell<NotifierHandle>,
    /// Lazily resolved, class-shared `eAllStructuralFeatures` + name index
    /// (see [`FeatureCache`]). Invalidated when the registry binding changes.
    feature_cache: OnceCell<Rc<FeatureCache>>,
}

impl DynamicEObject {
    /// New object of the given class, resolving inheritance against the
    /// process-wide global registry.
    pub fn new(class: EClass) -> Self {
        Self {
            e_class: shared_class(class),
            dynamic_settings: FeatureMap::default(),
            container: None,
            inverse_lists: InverseLists::default(),
            registry: None,
            proxy_uri: None,
            notifier: OnceCell::new(),
            feature_cache: OnceCell::new(),
        }
    }

    /// New object of the given class, binding it to a specific registry so the
    /// class's inheritance graph resolves against that registry.
    pub fn new_in(class: EClass, registry: crate::package::PackageRegistry) -> Self {
        Self {
            e_class: shared_class(class),
            dynamic_settings: FeatureMap::default(),
            container: None,
            inverse_lists: InverseLists::default(),
            registry: Some(registry),
            proxy_uri: None,
            notifier: OnceCell::new(),
            feature_cache: OnceCell::new(),
        }
    }

    /// Bind (or rebind) the registry used for inheritance resolution.
    pub fn bind_registry(&mut self, registry: crate::package::PackageRegistry) {
        self.registry = Some(registry);
        self.feature_cache = OnceCell::new();
    }

    /// The class descriptor.
    pub fn class(&self) -> &EClass {
        self.e_class.as_ref()
    }

    /// The notification sink (EMF `Notifier`) for this object, materializing it
    /// on first use.
    pub fn notifier(&self) -> &NotifierHandle {
        self.notifier.get_or_init(new_notifier_handle)
    }

    /// Attach an adapter to receive change notifications (EMF `eAdapters().add`).
    pub fn add_adapter(&self, adapter: Box<dyn Adapter>) {
        self.notifier().borrow_mut().add_adapter(adapter);
    }

    /// Detach adapter(s) matching a predicate (EMF `eAdapters().remove`). A
    /// never-materialized sink holds no adapters, so the call is a no-op.
    pub fn remove_adapter(&self, predicate: impl FnMut(&dyn Adapter) -> bool) {
        if let Some(notifier) = self.notifier.get() {
            notifier.borrow_mut().remove_adapter(predicate);
        }
    }

    /// Number of attached adapters (EMF `eAdapters().size`).
    pub fn adapter_count(&self) -> usize {
        self.notifier
            .get()
            .map_or(0, |n| n.borrow().adapters().len())
    }

    /// Whether changes must be delivered to adapters (C++
    /// `BasicEObject::eNotificationRequired`). Stays allocation-free: an
    /// unmaterialized sink can have neither adapters nor a disabled deliver
    /// flag, so it is never "required".
    pub fn e_notification_required(&self) -> bool {
        self.notifier
            .get()
            .is_some_and(|n| n.borrow().e_notification_required())
    }

    /// Register a reverse-reference list under `feature_id` (C++
    /// `BasicEObject::eRegisterInverseList`). `None` models a null list and is a
    /// no-op.
    pub fn e_register_inverse_list(
        &mut self,
        feature_id: i32,
        list: Option<Rc<RefCell<dyn InverseList>>>,
    ) {
        if let Some(list) = list {
            self.inverse_lists.register(feature_id, list);
        }
    }

    /// Unregister a reverse-reference list (C++
    /// `BasicEObject::eUnregisterInverseList`). Only removes the entry when it
    /// is exactly `list`; a mismatched list is a no-op.
    pub fn e_unregister_inverse_list(
        &mut self,
        feature_id: i32,
        list: &Rc<RefCell<dyn InverseList>>,
    ) {
        self.inverse_lists.unregister(feature_id, list);
    }

    /// The name of the structural feature with `feature_id`, if resolvable
    /// (C++ `eClass()->getEStructuralFeature(featureID)`); used to label the
    /// reverse notifications emitted by [`EObject::e_inverse_add`].
    fn feature_name_by_id(&self, feature_id: i32) -> Option<String> {
        if feature_id < 0 {
            return None;
        }
        self.features()
            .all
            .iter()
            .find(|f| f.feature_id() == feature_id)
            .map(|f| f.name().to_string())
    }

    /// Read a feature by name. Returns `None` only when the feature name is not
    /// part of `eAllStructuralFeatures`; otherwise returns the stored value or,
    /// failing that, the class default.
    pub fn e_get_by_name(&self, name: &str) -> Option<Val> {
        let fc = self.features();
        let i = *fc.by_name.get(name)?;
        Some(self.get_at_index(i, &fc.all[i]))
    }

    /// Read a feature's value (stored or default).
    pub fn e_get_feature(&self, feature: &crate::structural::EStructuralFeature) -> Val {
        // Resolve the feature to its slot in this class's feature list; a
        // feature the object's class does not declare falls back to its default.
        let fc = self.features();
        match fc.by_name.get(feature.name()) {
            Some(&i) => self.get_at_index(i, &fc.all[i]),
            None => default_value_of(feature),
        }
    }

    /// Read the value stored at feature slot `i`, or the feature's default.
    fn get_at_index(&self, i: usize, feature: &crate::structural::EStructuralFeature) -> Val {
        if let Some(v) = self.dynamic_settings.get(i as u32) {
            return v.clone();
        }
        default_value_of(feature)
    }

    /// The explicitly-set features as `(name, value)` pairs. Exposes the
    /// slot-keyed storage to callers that enumerate what an object holds; the
    /// order is feature-index order (see [`FeatureMap`]).
    pub fn settings(&self) -> Vec<(String, Val)> {
        let fc = self.features();
        self.dynamic_settings
            .iter()
            .filter_map(|(i, v)| {
                fc.all
                    .get(i as usize)
                    .map(|f| (f.name().to_string(), v.clone()))
            })
            .collect()
    }

    /// Set a feature by name. Returns false if the feature name is unknown.
    pub fn e_set_by_name(&mut self, name: &str, value: Val) -> bool {
        // Resolve the feature's kind from the (cached) feature index without
        // holding a borrow across the mutation below.
        let (slot, is_reference, upper_bound) = {
            let fc = self.features();
            match fc.by_name.get(name) {
                Some(&i) => {
                    let f = &fc.all[i];
                    (i as u32, f.is_reference(), f.upper_bound())
                }
                None => return false,
            }
        };
        // Reference features store object refs / object lists; attributes store
        // atomic values.
        let stored = if is_reference {
            normalize_reference_value(&value)
        } else {
            value
        };
        // Emit a SET notification for single-valued features only, mirroring the
        // C++ `DynamicEObject::eSet` (multi-valued writes go through the list and
        // return before notifying). Old value is the raw stored value (Null when
        // previously unset), matching C++.
        // Only build a notification when an adapter can actually receive it;
        // [`emit`] is a no-op otherwise, so materializing the feature name, the
        // old value and a clone of the new value would be pure overhead — which
        // dominated bulk loads where every leaf object sets a few features.
        let notify = upper_bound != -1 && self.e_notification_required();
        if notify {
            let old_value = self
                .dynamic_settings
                .insert(slot, stored.clone())
                .unwrap_or(Val::Null);
            let n = Notification::new(
                EventType::Set,
                Some(name.to_string()),
                old_value,
                stored,
                -1,
                false,
            );
            emit(self.notifier(), &n);
        } else {
            self.dynamic_settings.insert(slot, stored);
        }
        true
    }

    /// Append to a multi-valued feature in place. Returns false if the feature
    /// name is unknown. Unlike a read-modify-write through [`Self::e_set_by_name`]
    /// this never copies the existing list, so building a feature with many
    /// children stays linear instead of quadratic.
    pub fn e_append_by_name(&mut self, name: &str, value: Val) -> bool {
        let (slot, is_reference) = {
            let fc = self.features();
            match fc.by_name.get(name) {
                Some(&i) => (i as u32, fc.all[i].is_reference()),
                None => return false,
            }
        };
        let stored = if is_reference {
            normalize_reference_value(&value)
        } else {
            value
        };
        match self.dynamic_settings.get_mut(slot) {
            Some(Val::List(l)) => l.push(stored),
            _ => {
                self.dynamic_settings.insert(slot, Val::List(vec![stored]));
            }
        }
        true
    }

    /// Whether a feature (by name) is set.
    pub fn e_is_set_by_name(&self, name: &str) -> Option<bool> {
        let fc = self.features();
        let i = *fc.by_name.get(name)?;
        let feature = &fc.all[i];
        // Presence in `dynamic_settings` is the "set" state. A multi-valued
        // feature additionally requires a non-empty list (an explicitly set but
        // empty list reports unset, matching EMF); a single-valued feature
        // follows presence alone, so a value written even when equal to the
        // class default still reports set.
        let stored = self.dynamic_settings.get(i as u32);
        if feature.upper_bound() == -1 {
            Some(matches!(stored, Some(Val::List(l)) if !l.is_empty()))
        } else {
            Some(stored.is_some())
        }
    }

    /// Unset a feature by name, restoring the class default. Returns false if
    /// the name is unknown or the feature is unsettable-only.
    pub fn e_unset_by_name(&mut self, name: &str) -> bool {
        let (slot, is_containment) = {
            let fc = self.features();
            match fc.by_name.get(name) {
                Some(&i) => (i as u32, fc.all[i].is_containment()),
                None => return false,
            }
        };
        // Detach contained children before clearing.
        if is_containment {
            clear_container_children(self, name);
        }
        // As with `e_set_by_name`, only build/emit when an adapter can receive
        // it — this also keeps the lazy notifier unmaterialized on the bulk
        // paths where no sink was ever requested.
        let notify = self.e_notification_required();
        // Capture the raw old value before clearing for the UNSET notification
        // (mirrors C++ `DynamicEObject::eUnset`). Only needed when notifying.
        let old_value = if notify {
            self.dynamic_settings
                .get(slot)
                .cloned()
                .unwrap_or(Val::Null)
        } else {
            Val::Null
        };
        self.dynamic_settings.remove(slot);
        if notify {
            let n = Notification::new(
                EventType::Unset,
                Some(name.to_string()),
                old_value,
                Val::Null,
                -1,
                false,
            );
            emit(self.notifier(), &n);
        }
        true
    }

    // ---- container / containment graph ----

    /// Set (or clear with `None`) the weak back-link to this object's container.
    /// Called by a parent when it adopts this object through a containment ref.
    /// Internal: does not notify (C++ `eSetContainer`).
    pub fn set_container(&mut self, container: Option<ContainerBackref>) {
        self.container = container;
    }

    /// Set (or clear with `None`) this object's container, firing the reverse
    /// REMOVE(old)/ADD(new) notifications (C++ `EObjectImpl::setEContainer`).
    /// Assigning the same container is a no-op.
    pub fn set_e_container(&mut self, container: Option<ObjectRef>) {
        let same = match (&self.container, &container) {
            (None, None) => true,
            (Some((weak, _)), Some(new)) => weak.upgrade().is_some_and(|old| Rc::ptr_eq(&old, new)),
            _ => false,
        };
        if same {
            return;
        }
        let notify = self.e_notification_required();
        let old = self.container.take().and_then(|(weak, _)| weak.upgrade());
        // No containment feature is recorded here (C++ setEContainer does not
        // touch eContainingFeature), so the back-link keeps an empty feature.
        self.container = container
            .as_ref()
            .map(|c| (Rc::downgrade(c), String::new()));
        if !notify {
            return;
        }
        if let Some(old) = old {
            let r = Notification::new(
                EventType::Remove,
                None,
                Val::Object(old),
                Val::Null,
                -1,
                false,
            );
            emit(self.notifier(), &r);
        }
        if let Some(new) = container {
            let a = Notification::new(EventType::Add, None, Val::Null, Val::Object(new), -1, false);
            emit(self.notifier(), &a);
        }
    }

    /// The weak back-link to this object's container.
    pub fn container_backref(&self) -> Option<&ContainerBackref> {
        self.container.as_ref()
    }

    /// The containment children held by single and multi containment features,
    /// in feature-id order (mirrors C++ `eContents`).
    pub fn contents(&self) -> Vec<ObjectRef> {
        let fc = self.features();
        let mut out = Vec::new();
        for &i in &fc.containments {
            match self.dynamic_settings.get(i as u32) {
                Some(Val::Object(o)) => out.push(o.clone()),
                Some(Val::List(l)) => {
                    for e in l {
                        if let Some(o) = e.as_object() {
                            out.push(o.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The multi-valued list for a feature, expanding an unset default to an
    /// empty list so callers can populate it (mirrors C++ lazy list creation).
    pub fn e_list_mut(&mut self, name: &str) -> Vec<ObjectRef> {
        let slot = match self.features().by_name.get(name) {
            Some(&i) => i as u32,
            None => return Vec::new(),
        };
        let cur = self
            .dynamic_settings
            .get(slot)
            .cloned()
            .unwrap_or(Val::Null);
        cur.as_list()
            .map(|l| l.iter().filter_map(|v| v.as_object().cloned()).collect())
            .unwrap_or_default()
    }

    /// All structural features (own + inherited), using the bound registry or
    /// the global registry as fallback.
    pub fn all_structural_features(&self) -> Vec<crate::structural::EStructuralFeature> {
        self.e_all()
    }

    /// All structural features (own + inherited) borrowed from the per-class
    /// cache — no clone. Reflective hot paths (e.g. the per-object validation
    /// constraints) should prefer this over [`Self::all_structural_features`],
    /// whose `Vec` clone dominates for classes with deep inheritance.
    pub fn structural_features_ref(&self) -> &[crate::structural::EStructuralFeature] {
        &self.features().all
    }

    /// All reference features (own + inherited), for cross-reference / serialization.
    pub fn all_references(&self) -> Vec<crate::structural::EStructuralFeature> {
        self.features()
            .all
            .iter()
            .filter(|f| f.is_reference())
            .cloned()
            .collect()
    }

    /// All containment reference features (own + inherited), for serialization.
    pub fn all_containments(&self) -> Vec<crate::structural::EStructuralFeature> {
        self.features()
            .all
            .iter()
            .filter(|f| f.is_containment())
            .cloned()
            .collect()
    }

    /// The bound registry used to resolve inheritance, if any.
    pub fn registry(&self) -> Option<&crate::package::PackageRegistry> {
        self.registry.as_ref()
    }

    /// All structural features (own + inherited), using the bound registry or
    /// the global registry as fallback.
    fn e_all(&self) -> Vec<crate::structural::EStructuralFeature> {
        self.features().all.clone()
    }

    /// The cached `eAllStructuralFeatures` + name index for this object,
    /// resolved once (see [`FeatureCache`]). Binding a registry via
    /// [`Self::bind_registry`] clears it.
    fn features(&self) -> &FeatureCache {
        self.feature_cache.get_or_init(|| {
            let id = self.e_class.instance_id();
            CLASS_FEATURE_CACHE.with(|cell| {
                if let Some(cached) = cell.borrow().get(&id) {
                    return cached.clone();
                }
                // Borrow the bound registry directly instead of cloning it (the
                // fallback global snapshot is only taken for unbound objects).
                let all = match &self.registry {
                    Some(registry) => self.e_class.e_all_structural_features(registry),
                    None => {
                        let registry = crate::ecore_package::global();
                        self.e_class.e_all_structural_features(&registry)
                    }
                };
                // Index each feature under its registration name (the arxml
                // element name) plus its `ecore.name` alias, so reflective
                // lookups resolve both the arxml spelling (`SHORT-NAME`) and the
                // ecore spelling (`shortName`) the constraints use — matching
                // the C++ generated model, whose features are ecore-named.
                let mut by_name: FxHashMap<String, usize> =
                    FxHashMap::with_capacity_and_hasher(all.len() * 2, Default::default());
                for (i, f) in all.iter().enumerate() {
                    by_name.entry(f.name().to_string()).or_insert(i);
                    if let Some(alias) = f.tagged_value("ecore.name") {
                        by_name.entry(alias.to_string()).or_insert(i);
                    }
                }
                let containments = all
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.is_containment())
                    .map(|(i, _)| i)
                    .collect();
                let cache = Rc::new(FeatureCache {
                    all,
                    by_name,
                    containments,
                });
                cell.borrow_mut().insert(id, cache.clone());
                cache
            })
        })
    }
}

impl EObject for DynamicEObject {
    fn e_class(&self) -> &str {
        self.e_class.name()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clear_container(&mut self) {
        self.container = None;
    }

    fn e_container(&self) -> Option<ObjectRef> {
        self.container.as_ref().and_then(|(weak, _)| weak.upgrade())
    }

    fn e_containing_feature(&self) -> Option<String> {
        self.container.as_ref().and_then(|(weak, feature)| {
            weak.upgrade().and_then(|_| {
                // An empty feature marks a container set via `set_e_container`
                // (no containment feature recorded) -> report none, matching C++.
                if feature.is_empty() {
                    None
                } else {
                    Some(feature.clone())
                }
            })
        })
    }

    fn set_e_container(&mut self, container: Option<ObjectRef>) {
        DynamicEObject::set_e_container(self, container);
    }

    fn e_notification_required(&self) -> bool {
        DynamicEObject::e_notification_required(self)
    }

    fn e_inverse_add(
        &self,
        other_end: &ObjectRef,
        feature_id: i32,
        mut notifications: Vec<Notification>,
    ) -> Vec<Notification> {
        if let Some(list) = self.inverse_lists.get(feature_id) {
            list.borrow_mut().basic_add(other_end);
            if self.e_notification_required() {
                notifications.push(Notification::new(
                    EventType::Add,
                    self.feature_name_by_id(feature_id),
                    Val::Null,
                    Val::Object(Rc::clone(other_end)),
                    -1,
                    false,
                ));
            }
        }
        notifications
    }

    fn e_inverse_remove(
        &self,
        other_end: &ObjectRef,
        feature_id: i32,
        mut notifications: Vec<Notification>,
    ) -> Vec<Notification> {
        if let Some(list) = self.inverse_lists.get(feature_id) {
            list.borrow_mut().basic_remove(other_end);
            if self.e_notification_required() {
                notifications.push(Notification::new(
                    EventType::Remove,
                    self.feature_name_by_id(feature_id),
                    Val::Object(Rc::clone(other_end)),
                    Val::Null,
                    -1,
                    false,
                ));
            }
        }
        notifications
    }

    fn e_contents(&self) -> Vec<ObjectRef> {
        self.contents()
    }

    fn e_cross_references(&self) -> Vec<ObjectRef> {
        // Non-containment references' targets (EMF `eCrossReferences`), used by
        // reflective proxy checks (e.g. AUTOSAR no-unresolved-proxy). Only the
        // target objects held by *non-containment* reference features are
        // returned; containment children are already surfaced via `e_contents`.
        let fc = self.features();
        let mut out = Vec::new();
        for (i, f) in fc.all.iter().enumerate() {
            if !f.is_reference() || f.is_containment() {
                continue;
            }
            match self.dynamic_settings.get(i as u32) {
                Some(Val::Object(o)) => out.push(o.clone()),
                Some(Val::List(l)) => {
                    for e in l {
                        if let Some(o) = e.as_object() {
                            out.push(o.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn e_get(&self, feature_id: &str) -> Option<Val> {
        self.e_get_by_name(feature_id)
    }

    fn e_has_feature(&self, feature_id: &str) -> bool {
        self.features().by_name.contains_key(feature_id)
    }

    fn e_feature_lower_bound(&self, feature_id: &str) -> i32 {
        let fc = self.features();
        match fc.by_name.get(feature_id) {
            Some(&i) => fc.all[i].lower_bound(),
            None => 0,
        }
    }

    fn e_feature_upper_bound(&self, feature_id: &str) -> i32 {
        let fc = self.features();
        match fc.by_name.get(feature_id) {
            Some(&i) => fc.all[i].upper_bound(),
            None => -1,
        }
    }

    fn e_set(&mut self, feature_id: &str, value: Val) -> bool {
        self.e_set_by_name(feature_id, value)
    }

    fn e_append(&mut self, feature_id: &str, value: Val) -> bool {
        self.e_append_by_name(feature_id, value)
    }

    fn e_is_set(&self, feature_id: &str) -> bool {
        self.e_is_set_by_name(feature_id).unwrap_or(false)
    }

    fn e_unset(&mut self, feature_id: &str) -> bool {
        self.e_unset_by_name(feature_id)
    }

    fn e_proxy_uri(&self) -> Option<&Uri> {
        self.proxy_uri.as_ref()
    }

    fn e_set_proxy_uri(&mut self, uri: Option<Uri>) {
        self.proxy_uri = uri;
    }

    fn e_is_proxy(&self) -> bool {
        self.proxy_uri.is_some()
    }

    fn e_resolve_proxy(&self, proxy: &ObjectRef) -> ObjectRef {
        // Base contract (C++ `eResolveProxy`): a non-proxy resolves to itself.
        // When `proxy` is a real proxy with no resolving ResourceSet handy, the
        // degenerate result is the proxy unchanged. The ResourceSet-level
        // demand-load resolution lives on the XMI resource set (EMF `EcoreUtil`).
        Rc::clone(proxy)
    }
}

/// A fresh notifier handle with delivery enabled (EMF `Notifier` default).
fn new_notifier_handle() -> NotifierHandle {
    Rc::new(RefCell::new(Notifier::new()))
}

/// Normalize a caller-supplied reference value so it is stored as a proper
/// reference: single objects stay `Val::Object`, lists become `Val::List`.
fn normalize_reference_value(value: &Val) -> Val {
    match value {
        Val::List(_) => {
            let objs = value
                .as_list()
                .map(|l| {
                    l.iter()
                        .filter_map(|v| v.as_object().cloned())
                        .map(Val::Object)
                        .collect()
                })
                .unwrap_or_default();
            Val::List(objs)
        }
        other => other.clone(),
    }
}

/// The value a feature reads as when unset: an empty list for a many-valued
/// reference (EMF), otherwise the feature's declared default.
fn default_value_of(feature: &crate::structural::EStructuralFeature) -> Val {
    if feature.upper_bound() == -1 && feature.is_reference() {
        return Val::List(Vec::new());
    }
    feature.default_value().cloned().unwrap_or(Val::Null)
}

/// Detach the contained children of `name` from their container back-link.
fn clear_container_children(obj: &mut DynamicEObject, name: &str) {
    let slot = match obj.features().by_name.get(name) {
        Some(&i) => i as u32,
        None => return,
    };
    match obj.dynamic_settings.get(slot).cloned() {
        Some(Val::Object(o)) => {
            o.borrow_mut().clear_container();
        }
        Some(Val::List(l)) => {
            let objs: Vec<ObjectRef> = l.iter().filter_map(|v| v.as_object().cloned()).collect();
            for o in objs {
                o.borrow_mut().clear_container();
            }
        }
        _ => {}
    }
}

/// A shared handle to a `DynamicEObject` for graph building (containment).
pub type DynNode = Rc<RefCell<DynamicEObject>>;

/// Coerce a `DynNode` to a type-erased `ObjectRef` (`dyn EObject`).
pub fn node_to_object(node: &DynNode) -> ObjectRef {
    node.clone() as ObjectRef
}

/// Adopt `child` into `parent` through a single containment feature `name`:
/// records the value on the parent and sets the child's weak container back-link.
pub fn adopt_single(parent: &DynNode, name: &str, child: &DynNode) {
    let weak = Rc::downgrade(&(parent.clone() as ObjectRef));
    parent
        .borrow_mut()
        .e_set_by_name(name, Val::Object(node_to_object(child)));
    child
        .borrow_mut()
        .set_container(Some((weak, name.to_string())));
}

/// Adopt `child` into `parent` through a multi containment feature `name`,
/// appending to the stored list and setting the child's container back-link.
pub fn adopt_many(parent: &DynNode, name: &str, child: &DynNode) {
    let weak = Rc::downgrade(&(parent.clone() as ObjectRef));
    {
        let mut p = parent.borrow_mut();
        let mut list = p.e_list_mut(name);
        list.push(node_to_object(child));
        p.e_set_by_name(name, Val::List(list.into_iter().map(Val::Object).collect()));
    }
    child
        .borrow_mut()
        .set_container(Some((weak, name.to_string())));
}
