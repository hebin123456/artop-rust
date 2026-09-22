//! FeatureID constants for the Ecore meta-meta-model.
//!
//! These are the integer feature indexes used by EMF's generated switch-style
//! `eGet`/`eSet`/`eIsSet`/`eUnset` and by `DynamicEObject` storage. They mirror
//! the `EMF_ ? ` block exactly as in C++ `EcorePackage.h` `FeatureID`.

/// Named constants for every structural/operation feature of the Ecore
/// meta-meta-model, following C++ `emf-common::FeatureID`.
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)] // mirrors the Java `FeatureID` constant group naming.
pub mod FeatureID {
    /// EModelElement.eAnnotations.
    pub const EMODEL_ELEMENT_EANNOTATIONS: i32 = 0;
    /// ENamedElement.name.
    pub const ENAMED_ELEMENT_ENAME: i32 = 1000;

    /// ETypedElement.lowerBound.
    pub const ETYPED_ELEMENT_ELOWERBOUND: i32 = 2000;
    /// ETypedElement.upperBound.
    pub const ETYPED_ELEMENT_EUPPERBOUND: i32 = 2001;
    /// ETypedElement.ordered.
    pub const ETYPED_ELEMENT_EORDERED: i32 = 2002;
    /// ETypedElement.unique.
    pub const ETYPED_ELEMENT_EUNIQUE: i32 = 2003;
    /// ETypedElement.eType.
    pub const ETYPED_ELEMENT_ETYPE: i32 = 2004;
    /// ETypedElement.eGenericType.
    pub const ETYPED_ELEMENT_EGENERICTYPE: i32 = 2005;

    /// EClassifier.instanceClassName.
    pub const ECLASSIFIER_EINSTANCECLASSNAME: i32 = 3000;
    /// EClassifier.defaultValue.
    pub const ECLASSIFIER_EDEFAULTVALUE: i32 = 3001;
    /// EClassifier.eTypeParameters.
    pub const ECLASSIFIER_ETYPEPARAMETERS: i32 = 3002;

    /// EClass.abstract.
    pub const ECLASS_EABSTRACT: i32 = 4000;
    /// EClass.interface.
    pub const ECLASS_EINTERFACE: i32 = 4001;
    /// EClass.eSuperTypes.
    pub const ECLASS_ESUPERTYPES: i32 = 4002;
    /// EClass.eStructuralFeatures.
    pub const ECLASS_ESTRUCTURALFEATURES: i32 = 4003;
    /// EClass.eOperations.
    pub const ECLASS_EOPERATIONS: i32 = 4004;
    /// EClass.eAllAttributes (derived).
    pub const ECLASS_EALLATTRIBUTES: i32 = 4005;
    /// EClass.eAllReferences (derived).
    pub const ECLASS_EALLREFERENCES: i32 = 4006;
    /// EClass.eAttributes (derived).
    pub const ECLASS_EATTRIBUTES: i32 = 4007;
    /// EClass.eReferences (derived).
    pub const ECLASS_EREFERENCES: i32 = 4008;
    /// EClass.eAllOperations (derived).
    pub const ECLASS_EALLOPERATIONS: i32 = 4009;
    /// EClass.eAllStructuralFeatures (derived).
    pub const ECLASS_EALLSTRUCTURALFEATURES: i32 = 4010;
    /// EClass.eGenericSuperTypes.
    pub const ECLASS_EGENERICSUPERTYPES: i32 = 4011;

    /// EDataType.serializable.
    pub const EDATATYPE_ESERIALIZABLE: i32 = 5000;
    /// EEnum.eLiterals.
    pub const EENUM_ELITERALS: i32 = 6000;

    /// EEnumLiteral.value.
    pub const EENUMLITERAL_EVALUE: i32 = 7000;
    /// EEnumLiteral.literal.
    pub const EENUMLITERAL_ELITERAL: i32 = 7001;
    /// EEnumLiteral.eInstance.
    pub const EENUMLITERAL_EINSTANCE: i32 = 7002;
    /// EEnumLiteral.eEnum.
    pub const EENUMLITERAL_EENUM: i32 = 7003;

    /// EFactory.ePackage.
    pub const EFACTORY_EPACKAGE: i32 = 8000;
    /// EOperation.eParameters.
    pub const EOPERATION_EPARAMETERS: i32 = 9000;
    /// EOperation.eExceptions.
    pub const EOPERATION_EEXCEPTIONS: i32 = 9001;
    /// EOperation.body.
    pub const EOPERATION_EBODY: i32 = 9002;
    /// EOperation.eTypeParameters.
    pub const EOPERATION_ETYPEPARAMETERS: i32 = 9003;

    /// EPackage.nsURI.
    pub const EPACKAGE_ENSURI: i32 = 10000;
    /// EPackage.nsPrefix.
    pub const EPACKAGE_ENSPREFIX: i32 = 10001;
    /// EPackage.eClassifiers.
    pub const EPACKAGE_ECLASSIFIERS: i32 = 10002;
    /// EPackage.eFactoryInstance.
    pub const EPACKAGE_EFACTORYINSTANCE: i32 = 10003;
    /// EPackage.eSuperPackage.
    pub const EPACKAGE_ESUPERPACKAGE_NEW: i32 = 10004;
    /// EPackage.eSubPackages.
    pub const EPACKAGE_ESUBPACKAGES: i32 = 10005;

    /// EStructuralFeature.featureID.
    pub const ESTRUCTURALFEATURE_EFEATUREID: i32 = 11000;
    /// EStructuralFeature.changeable.
    pub const ESTRUCTURALFEATURE_ECHANGEABLE: i32 = 11001;
    /// EStructuralFeature.volatile.
    pub const ESTRUCTURALFEATURE_EVOLATILE: i32 = 11002;
    /// EStructuralFeature.transient.
    pub const ESTRUCTURALFEATURE_ETRANSIENT: i32 = 11003;
    /// EStructuralFeature.unsettable.
    pub const ESTRUCTURALFEATURE_EUNSETTABLE: i32 = 11004;
    /// EStructuralFeature.derived.
    pub const ESTRUCTURALFEATURE_EDERIVED: i32 = 11005;
    /// EStructuralFeature.defaultValueLiteral.
    pub const ESTRUCTURALFEATURE_EDEFAULTVALUELITERAL: i32 = 11006;
    /// EStructuralFeature.eContainingClass.
    pub const ESTRUCTURALFEATURE_EECONTAININGCLASS: i32 = 11007;

    /// EAttribute.eAttributeType.
    pub const EATTRIBUTE_EATTRIBUTETYPE: i32 = 12000;
    /// EAttribute.ID.
    pub const EATTRIBUTE_EID: i32 = 12001;

    /// EReference.eReferenceType.
    pub const EREFERENCE_EREFERENCETYPE: i32 = 13000;
    /// EReference.eOpposite.
    pub const EREFERENCE_EOPPOSITE: i32 = 13001;
    /// EReference.containment.
    pub const EREFERENCE_ECONTAINMENT: i32 = 13002;
    /// EReference.container.
    pub const EREFERENCE_ECONTAINER: i32 = 13003;
    /// EReference.resolveProxies.
    pub const EREFERENCE_ERESOLVEPROXIES: i32 = 13004;

    /// EParameter.eOperation.
    pub const EPARAMETER_EOPERATION: i32 = 14000;

    /// EGenericType.eClassifier.
    pub const EGENERICTYPE_ECLASSIFIER: i32 = 15000;
    /// EGenericType.eTypeArguments.
    pub const EGENERICTYPE_ETYPEARGUMENTS: i32 = 15001;
    /// EGenericType.eUpperBound.
    pub const EGENERICTYPE_EUPPERBOUND: i32 = 15002;
    /// EGenericType.eLowerBound.
    pub const EGENERICTYPE_ELOWERBOUND: i32 = 15003;
    /// EGenericType.eTypeParameter.
    pub const EGENERICTYPE_ETYPEPARAMETER: i32 = 15004;

    /// EAnnotation.source.
    pub const EANNOTATION_ESOURCE: i32 = 16000;
    /// EAnnotation.details.
    pub const EANNOTATION_EDETAILS: i32 = 16001;
    /// EAnnotation.eContents.
    pub const EANNOTATION_ECONTENTS: i32 = 16002;
    /// EAnnotation.eReferences.
    pub const EANNOTATION_EREFERENCES: i32 = 16003;
    /// EAnnotation.eModelElement.
    pub const EANNOTATION_EMODEL_ELEMENT: i32 = 16004;

    /// ETypeParameter.eBounds.
    pub const ETYPEPARAMETER_EBOUNDS: i32 = 17000;

    /// The first "free" FeatureID assigned to custom classifier features.
    pub const CUSTOM_BASE: i32 = 100000;
}
