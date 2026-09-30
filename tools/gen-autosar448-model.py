#!/usr/bin/env python3
"""Regenerate `crates/emf-artop/autosar448-model/src/registry.rs` from the AUTOSAR `.ecore` sources.

Usage:
    python3 tools/gen-autosar448-model.py <gautosar.ecore> <autosar448.ecore> [out.rs]

The AUTOSAR 4.4.8 metamodel is split across two ecore files:
  * `gautosar.ecore`    — the *generic* AUTOSAR layer (`g*` base classes such as
    `GARObject`, `GReferrable`, `GIdentifiable`).
  * `autosar448.ecore`  — the full AUTOSAR 4.4.8 model, whose base classes
    (`ARObject`, `Referrable`, ...) inherit from the generic layer via
    `gautosar.ecore#//...`.

Both are merged into one flat registry so that inheritance and reflection are
pure data. For every `EClass` we emit its `eSuperTypes` graph, its own
`EStructuralFeature`s and the *serialization metadata* the arxml layer needs
(`xml.name`, `xml.namePlural`, `internal-xml-sequenceOffset`, the APRXML
role/type element flags, containment/multiplicity, ...), mirroring the C++
`emf::ecore::codegen::EAnnotationReader` extraction exactly.

If `out.rs` is given the file is written, otherwise the source is printed.
"""
import sys
import xml.etree.ElementTree as ET

XSI = "http://www.w3.org/2001/XMLSchema-instance"
EMD = "http:///org/eclipse/emf/ecore/util/ExtendedMetaData"


def q(tag):
    return tag.split("}")[-1]


def xsi_type(elem):
    return elem.get("{%s}type" % XSI, "")


def details(elem, source):
    """Details of the eAnnotations with the given source ({} if absent)."""
    for a in elem:
        if q(a.tag) == "eAnnotations" and a.get("source") == source:
            d = {}
            for dd in a:
                if q(dd.tag) == "details":
                    d[dd.get("key")] = dd.get("value") or ""
            return d
    return {}


def parse_bool(s):
    return s in ("true", "1", "True", "TRUE")


def parse_int(s):
    if not s:
        return 0
    try:
        return int(s)
    except ValueError:
        return 0


class Package:
    def __init__(self, name, qualified):
        self.name = name
        self.qualified = qualified      # "genericstructure/generaltemplateclasses/identifiable"
        self.ns_uri = ""
        self.ns_prefix = ""


class Class:
    def __init__(self):
        self.name = ""
        self.xml_name = ""
        self.xml_name_plural = ""
        self.content_kind = ""
        self.qualified = ""
        self.pkg = 0
        self.abstract = False
        self.supers = []        # qualified paths as written in the ecore
        self.features = []      # list[Feature]


class Enum:
    def __init__(self):
        self.name = ""
        self.qualified = ""
        self.pkg = 0
        self.literals = []


class DataType:
    def __init__(self):
        self.name = ""
        self.qualified = ""
        self.pkg = 0
        self.instance_class = ""


class Feature:
    __slots__ = (
        "name", "kind", "e_type", "containment", "lower", "upper",
        "transient", "volatile", "derived", "default_value",
        "xml_name", "xml_name_plural", "xml_attribute", "text_content",
        "seq_offset", "role_element", "role_wrapper", "type_element",
        "type_wrapper", "feature_kind", "ns_prefix",
    )


def parse_feature(el):
    f = Feature()
    f.name = el.get("name") or ""
    t = xsi_type(el)
    f.kind = "Reference" if t == "ecore:EReference" else "Attribute"
    f.e_type = el.get("eType") or ""
    for c in el:
        if q(c.tag) == "eType":
            f.e_type = c.get("href") or c.get("eType") or f.e_type
    f.containment = parse_bool(el.get("containment", ""))
    f.lower = parse_int(el.get("lowerBound")) if el.get("lowerBound") is not None else 0
    f.upper = parse_int(el.get("upperBound")) if el.get("upperBound") is not None else 1
    f.transient = parse_bool(el.get("transient", ""))
    f.volatile = parse_bool(el.get("volatile", ""))
    f.derived = parse_bool(el.get("derived", ""))
    f.default_value = el.get("defaultValueLiteral") or ""

    tv = details(el, "TaggedValues")
    emd = details(el, EMD)
    f.xml_name = tv.get("xml.name") or ""
    f.xml_name_plural = tv.get("xml.namePlural") or ""
    f.feature_kind = emd.get("kind") or ""
    seq = tv.get("internal-xml-sequenceOffset") or ""
    if not seq or seq == "null":
        seq = tv.get("xml.sequenceOffset") or ""
    f.seq_offset = parse_int(seq)
    f.role_element = parse_bool(tv.get("xml.roleElement", ""))
    f.role_wrapper = parse_bool(tv.get("xml.roleWrapperElement", ""))
    f.type_element = parse_bool(tv.get("xml.typeElement", ""))
    f.type_wrapper = parse_bool(tv.get("xml.typeWrapperElement", ""))
    f.xml_attribute = parse_bool(tv.get("xml.attribute", ""))
    f.text_content = parse_bool(tv.get("xml.text", ""))
    f.ns_prefix = tv.get("xml.nsPrefix") or ""
    if not f.xml_name:
        f.xml_name = f.name
    if not f.xml_name_plural:
        f.xml_name_plural = f.xml_name
    return f


def walk_packages(elem, packages, classes, enums, datatypes, parent_qualified):
    for ch in elem:
        t = q(ch.tag)
        if t == "eSubpackages":
            name = ch.get("name") or ""
            qual = (parent_qualified + "/" + name) if parent_qualified else name
            p = Package(name, qual)
            tv = details(ch, "TaggedValues")
            p.ns_uri = ch.get("nsURI") or tv.get("xml.nsUri") or ""
            p.ns_prefix = ch.get("nsPrefix") or tv.get("xml.nsPrefix") or ""
            packages.append(p)
            walk_packages(ch, packages, classes, enums, datatypes, qual)
        elif t == "eClassifiers":
            xt = xsi_type(ch)
            name = ch.get("name") or ""
            qual = (parent_qualified + "/" + name) if parent_qualified else name
            if xt == "ecore:EClass":
                c = Class()
                c.name = name
                c.qualified = qual
                c.abstract = parse_bool(ch.get("abstract", ""))
                tv = details(ch, "TaggedValues")
                emd = details(ch, EMD)
                c.xml_name = tv.get("xml.name") or name
                c.xml_name_plural = tv.get("xml.namePlural") or c.xml_name
                c.content_kind = emd.get("kind") or ""
                supers = (ch.get("eSuperTypes") or "").split()
                for cc in ch:
                    if q(cc.tag) == "eSuperTypes":
                        supers.append(cc.get("href") or cc.get("eType") or "")
                c.supers = [s for s in supers if s]
                for cc in ch:
                    if q(cc.tag) == "eStructuralFeatures":
                        c.features.append(parse_feature(cc))
                classes.append(c)
            elif xt == "ecore:EEnum":
                e = Enum()
                e.name = name
                e.qualified = qual
                for cc in ch:
                    if q(cc.tag) == "eLiterals":
                        e.literals.append(cc.get("name") or "")
                enums.append(e)
            elif xt == "ecore:EDataType":
                d = DataType()
                d.name = name
                d.qualified = qual
                d.instance_class = ch.get("instanceClassName") or ""
                datatypes.append(d)


def leaf(ref):
    """Last path segment of an ecore reference ('#//a/b/C' or 'file#//a/b/C' -> 'C')."""
    return (ref.split("#//")[-1] if "#//" in ref else ref).split("/")[-1].strip()


def rlit(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def gen(gautosar_path, autosar_path):
    packages, classes, enums, datatypes = [], [], [], []
    for path in (gautosar_path, autosar_path):
        root = ET.parse(path).getroot()
        name = root.get("name") or ""
        tv = details(root, "TaggedValues")
        p = Package(name, "")
        p.ns_uri = root.get("nsURI") or tv.get("xml.nsUri") or ""
        p.ns_prefix = root.get("nsPrefix") or tv.get("xml.nsPrefix") or ""
        packages.append(p)
        walk_packages(root, packages, classes, enums, datatypes, "")

    # ---- index by simple name (the two files use disjoint class names) ----
    class_by_name = {}
    for i, c in enumerate(classes):
        class_by_name.setdefault(c.name, i)
    enum_by_name = {}
    for i, e in enumerate(enums):
        enum_by_name.setdefault(e.name, i)
    dt_by_name = {}
    for i, d in enumerate(datatypes):
        dt_by_name.setdefault(d.name, i)
    pkg_by_qual = {}
    for i, p in enumerate(packages):
        pkg_by_qual.setdefault(p.qualified, i)

    def package_id(qualified):
        return pkg_by_qual.get(qualified, 0)

    for c in classes:
        c.pkg = package_id(c.qualified.rsplit("/", 1)[0] if "/" in c.qualified else "")
    for e in enums:
        e.pkg = package_id(e.qualified.rsplit("/", 1)[0] if "/" in e.qualified else "")
    for d in datatypes:
        d.pkg = package_id(d.qualified.rsplit("/", 1)[0] if "/" in d.qualified else "")

    # ---- resolve supers ----
    sup_idx = []
    for c in classes:
        r = set()
        for s in c.supers:
            nm = leaf(s)
            if nm in class_by_name:
                i = class_by_name[nm]
                if i != class_by_name[c.name]:
                    r.add(i)
        sup_idx.append(sorted(r))

    # ---- flatten features (class-qualified; ids are global) ----
    feature_index = {}
    feat_rows = []          # list[(Class, Feature)]
    own_ids = []
    for c in classes:
        ids = []
        for f in c.features:
            key = (c.name, f.name)
            fid = feature_index.get(key)
            if fid is None:
                fid = len(feat_rows)
                feature_index[key] = fid
                feat_rows.append(f)
            ids.append(fid)
        own_ids.append(ids)

    unresolved = {}

    # Built-in EMF/ecore types referenced by hand-written features (e.g. the
    # FeatureMap `mixed`/`extensions` slots). They have no AUTOSAR datatype
    # entry; resolve to `TypeRef::None` (the loader treats them as opaque text).
    EMF_BUILTIN = {
        "EFeatureMapEntry", "EStringToStringMapEntry", "EObject", "EJavaObject",
        "EString", "EInt", "EBoolean", "EDouble", "ELong", "EShort", "EByte",
        "EChar", "EFloat", "EBigInteger", "EBigDecimal", "EDate", "EAny",
    }

    def type_ref(f):
        nm = leaf(f.e_type)
        if f.kind == "Attribute":
            if nm in dt_by_name:
                return "TypeRef::DataType(%d)" % dt_by_name[nm]
            if nm in enum_by_name:
                return "TypeRef::Enum(%d)" % enum_by_name[nm]
            if nm in EMF_BUILTIN:
                return "TypeRef::None"
            unresolved[nm] = unresolved.get(nm, 0) + 1
            return "TypeRef::None"
        # Reference
        if nm in class_by_name:
            return "TypeRef::Class(%d)" % class_by_name[nm]
        if nm in EMF_BUILTIN:
            return "TypeRef::None"
        unresolved[nm] = unresolved.get(nm, 0) + 1
        return "TypeRef::None"

    order = sorted(range(len(classes)), key=lambda i: classes[i].name)
    n = len(classes)
    nf = len(feat_rows)

    out = []
    out.append("// AUTO-GENERATED from gautosar.ecore + autosar448.ecore — do not edit")
    out.append("// (regenerate with tools/gen-autosar448-model.py)")
    out.append("")
    out.append("pub const N_PACKAGES: usize = %d;" % len(packages))
    out.append("pub const N_CLASSES: usize = %d;" % n)
    out.append("pub const N_FEATURES: usize = %d;" % nf)
    out.append("pub const N_ENUMS: usize = %d;" % len(enums))
    out.append("pub const N_DATATYPES: usize = %d;" % len(datatypes))
    out.append("")

    # ---- packages ----
    out.append("#[derive(Clone, Copy, Debug)]")
    out.append("pub struct PackageMeta {")
    out.append("    pub name: &'static str,")
    out.append("    pub ns_uri: &'static str,")
    out.append("    pub ns_prefix: &'static str,")
    out.append("}")
    out.append("#[rustfmt::skip]")
    out.append("pub static PACKAGES: [PackageMeta; %d] = [" % len(packages))
    for p in packages:
        out.append("    PackageMeta { name: %s, ns_uri: %s, ns_prefix: %s },"
                   % (rlit(p.name), rlit(p.ns_uri), rlit(p.ns_prefix)))
    out.append("];")
    out.append("")

    # ---- enums ----
    out.append("#[derive(Clone, Copy, Debug)]")
    out.append("pub struct EnumMeta {")
    out.append("    pub name: &'static str,")
    out.append("    pub package: u32,")
    out.append("    pub literals: &'static [&'static str],")
    out.append("}")
    out.append("#[rustfmt::skip]")
    out.append("pub static ENUMS: [EnumMeta; %d] = [" % len(enums))
    for e in enums:
        out.append("    EnumMeta { name: %s, package: %d, literals: &[%s] },"
                   % (rlit(e.name), e.pkg, ", ".join(rlit(l) for l in e.literals)))
    out.append("];")
    out.append("#[rustfmt::skip]")
    out.append("pub static ENUM_NAME_TO_ID: [(&str, u32); %d] = [" % len(enums))
    for i in sorted(range(len(enums)), key=lambda i: enums[i].name):
        out.append("    (%s, %d)," % (rlit(enums[i].name), i))
    out.append("];")
    out.append("")

    # ---- datatypes ----
    out.append("#[derive(Clone, Copy, Debug)]")
    out.append("pub struct DataTypeMeta {")
    out.append("    pub name: &'static str,")
    out.append("    pub package: u32,")
    out.append("    pub instance_class: &'static str,")
    out.append("}")
    out.append("#[rustfmt::skip]")
    out.append("pub static DATATYPES: [DataTypeMeta; %d] = [" % len(datatypes))
    for d in datatypes:
        out.append("    DataTypeMeta { name: %s, package: %d, instance_class: %s },"
                   % (rlit(d.name), d.pkg, rlit(d.instance_class)))
    out.append("];")
    out.append("")

    # ---- feature type/kind enums + meta ----
    out.append("/// Whether a feature is an `EAttribute` (scalar text) or an `EReference`.")
    out.append("#[derive(Clone, Copy, PartialEq, Eq, Debug)]")
    out.append("pub enum FeatureKind { Attribute, Reference }")
    out.append("")
    out.append("/// Resolved `eType` of a feature.")
    out.append("#[derive(Clone, Copy, PartialEq, Eq, Debug)]")
    out.append("pub enum TypeRef { None, Class(u32), Enum(u32), DataType(u32) }")
    out.append("")
    out.append("/// Serialization metadata for one `EStructuralFeature`.")
    out.append("#[derive(Clone, Copy, Debug)]")
    out.append("pub struct FeatureMeta {")
    for fld, ty in [
        ("name", "&'static str"),
        ("xml_name", "&'static str"),
        ("xml_name_plural", "&'static str"),
        ("feature_kind", "&'static str"),
        ("kind", "FeatureKind"),
        ("ty", "TypeRef"),
        ("containment", "bool"),
        ("lower", "i32"),
        ("upper", "i32"),
        ("transient", "bool"),
        ("volatile_", "bool"),
        ("derived", "bool"),
        ("xml_attribute", "bool"),
        ("ns_prefix", "&'static str"),
        ("text_content", "bool"),
        ("seq_offset", "i32"),
        ("role_element", "bool"),
        ("role_wrapper", "bool"),
        ("type_element", "bool"),
        ("type_wrapper", "bool"),
        ("default_value", "&'static str"),
    ]:
        out.append("    pub %s: %s," % (fld, ty))
    out.append("}")
    out.append("#[rustfmt::skip]")
    out.append("pub static FEATURE_META: [FeatureMeta; %d] = [" % nf)
    for f in feat_rows:
        kind = "FeatureKind::%s" % f.kind
        out.append(
            "    FeatureMeta { name: %s, xml_name: %s, xml_name_plural: %s, feature_kind: %s, "
            "kind: %s, ty: %s, containment: %s, lower: %d, upper: %d, transient: %s, "
            "volatile_: %s, derived: %s, xml_attribute: %s, ns_prefix: %s, text_content: %s, seq_offset: %d, "
            "role_element: %s, role_wrapper: %s, type_element: %s, type_wrapper: %s, "
            "default_value: %s },"
            % (rlit(f.name), rlit(f.xml_name), rlit(f.xml_name_plural), rlit(f.feature_kind),
               kind, type_ref(f),
               "true" if f.containment else "false", f.lower, f.upper,
               "true" if f.transient else "false", "true" if f.volatile else "false",
               "true" if f.derived else "false", "true" if f.xml_attribute else "false",
               rlit(f.ns_prefix),
               "true" if f.text_content else "false", f.seq_offset,
               "true" if f.role_element else "false", "true" if f.role_wrapper else "false",
               "true" if f.type_element else "false", "true" if f.type_wrapper else "false",
               rlit(f.default_value)))
    out.append("];")
    out.append("")

    # ---- classes ----
    out.append("/// Static metadata for one `EClass`.")
    out.append("#[derive(Clone, Copy, Debug)]")
    out.append("pub struct ClassMeta {")
    out.append("    pub name: &'static str,")
    out.append("    pub xml_name: &'static str,")
    out.append("    pub xml_name_plural: &'static str,")
    out.append("    pub abstract_: bool,")
    out.append("    pub package: u32,")
    out.append("    pub content_kind: &'static str,")
    out.append("    pub sups: &'static [u32],")
    out.append("    pub own: &'static [u32],")
    out.append("}")
    out.append("#[rustfmt::skip]")
    out.append("pub static ECLASS: [ClassMeta; %d] = [" % n)
    for i, c in enumerate(classes):
        sups = "&[" + ",".join(str(s) for s in sup_idx[i]) + "]"
        own = "&[" + ",".join(str(x) for x in own_ids[i]) + "]"
        out.append(
            "    ClassMeta { name: %s, xml_name: %s, xml_name_plural: %s, abstract_: %s, "
            "package: %d, content_kind: %s, sups: %s, own: %s },"
            % (rlit(c.name), rlit(c.xml_name), rlit(c.xml_name_plural),
               "true" if c.abstract else "false", c.pkg, rlit(c.content_kind), sups, own))
    out.append("];")
    out.append("")

    # ---- name index + accessors ----
    out.append("#[rustfmt::skip]")
    out.append("pub static NAME_TO_ID: [(&str, u32); %d] = [" % n)
    for i in order:
        out.append("    (%s, %d)," % (rlit(classes[i].name), i))
    out.append("];")
    out.append("#[rustfmt::skip]")
    out.append("pub static XML_NAME_TO_ID: [(&str, u32); %d] = [" % n)
    for i in sorted(range(n), key=lambda i: classes[i].xml_name):
        out.append("    (%s, %d)," % (rlit(classes[i].xml_name), i))
    out.append("];")
    out.append("")
    out.append("/// Look up a class id by simple ecore name (binary search).")
    out.append("pub fn name_to_id(name: &str) -> Option<u32> {")
    out.append("    NAME_TO_ID")
    out.append("        .binary_search_by(|(n, _)| n.cmp(&name))")
    out.append("        .ok()")
    out.append("        .map(|i| NAME_TO_ID[i].1)")
    out.append("}")
    out.append("")
    out.append("/// Look up a class id by arxml element name (`xml.name`).")
    out.append("pub fn xml_name_to_id(xml: &str) -> Option<u32> {")
    out.append("    XML_NAME_TO_ID")
    out.append("        .binary_search_by(|(n, _)| n.cmp(&xml))")
    out.append("        .ok()")
    out.append("        .map(|i| XML_NAME_TO_ID[i].1)")
    out.append("}")
    out.append("")
    out.append("/// Look up an enum id by name.")
    out.append("pub fn enum_id(name: &str) -> Option<u32> {")
    out.append("    ENUM_NAME_TO_ID")
    out.append("        .binary_search_by(|(n, _)| n.cmp(&name))")
    out.append("        .ok()")
    out.append("        .map(|i| ENUM_NAME_TO_ID[i].1)")
    out.append("}")
    out.append("")
    out.append("/// Enum metadata by id.")
    out.append("pub fn enum_meta(id: u32) -> EnumMeta { ENUMS[id as usize] }")
    out.append("")
    out.append("/// Data type metadata by id.")
    out.append("pub fn datatype_meta(id: u32) -> DataTypeMeta { DATATYPES[id as usize] }")
    out.append("")

    diag = "unresolved eTypes: " + (", ".join("%s(%d)" % (k, v) for k, v in
                                              sorted(unresolved.items())) or "none")
    return "\n".join(out), dict(
        packages=len(packages), classes=n, features=nf, enums=len(enums),
        datatypes=len(datatypes), unresolved=unresolved, diag=diag,
        max_sup=max((len(s) for s in sup_idx), default=0),
    )


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit("usage: gen-autosar448-model.py <gautosar.ecore> <autosar448.ecore> [out.rs]")
    text, stats = gen(sys.argv[1], sys.argv[2])
    if len(sys.argv) > 3:
        with open(sys.argv[3], "w") as f:
            f.write(text)
        print("wrote", sys.argv[3])
    else:
        sys.stdout.write(text)
    print("stats:", stats, file=sys.stderr)