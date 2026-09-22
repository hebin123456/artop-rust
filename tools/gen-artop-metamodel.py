#!/usr/bin/env python3
"""Regenerate `crates/artop-metamodel/src/registry.rs` from an AUTOSAR `.ecore`.

Usage:
    python3 tools/gen-artop-metamodel.py <path-to-autosar448.ecore> [out.rs]

If `out.rs` is given the file is written, otherwise the generated Rust source is
printed to stdout. Reproduces the same metadata compiled into the repo.
"""
import sys
import xml.etree.ElementTree as ET

XSI = "http://www.w3.org/2001/XMLSchema-instance"


def q(tag):
    return tag.split("}")[-1]


def walk_classifiers(elem, out):
    for ch in elem:
        t = q(ch.tag)
        if t == "eClassifiers":
            if ch.get("{%s}type" % XSI, "") == "ecore:EClass":
                supers = (ch.get("eSuperTypes") or "").split()
                for cc in ch:
                    if q(cc.tag) == "eSuperTypes":
                        supers.append(cc.get("href") or cc.get("eType") or "")
                own = [
                    cc.get("name")
                    for cc in ch
                    if q(cc.tag) == "eStructuralFeatures" and cc.get("name")
                ]
                out.append(
                    dict(
                        name=ch.get("name"),
                        abstract=(ch.get("abstract") == "true"),
                        supers=supers,
                        own=own,
                    )
                )
        elif t == "eSubpackages":
            walk_classifiers(ch, out)


def rlit(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def gen(ecore_path):
    root = ET.parse(ecore_path).getroot()
    classes = []
    walk_classifiers(root, classes)
    seen = {}
    for c in classes:
        seen.setdefault(c["name"], c)
    classes = list(seen.values())
    index = {c["name"]: i for i, c in enumerate(classes)}

    sup_idx = []
    for c in classes:
        r = set()
        for s in c["supers"]:
            nm = (s.split("#//")[-1] if "#//" in s else s).split("/")[-1].strip()
            if nm in index:
                r.add(index[nm])
        sup_idx.append([s for s in sorted(r) if s != index[c["name"]]])

    # global feature table
    feat_ids, feat_names = {}, []
    for c in classes:
        for f in c["own"]:
            if f not in feat_ids:
                feat_ids[f] = len(feat_names)
                feat_names.append(f)

    order = sorted(range(len(classes)), key=lambda i: classes[i]["name"])
    n = len(classes)

    out = ["// AUTO-GENERATED from autosar448.ecore — do not edit (run tools/gen-artop-metamodel.py)"]
    out.append(f"pub const N_CLASSES: usize = {n};")
    out.append(f"pub const N_FEATURES: usize = {len(feat_names)};")
    out.append("#[rustfmt::skip]")
    out.append(f"pub static FEATURE_NAMES: [&str; {len(feat_names)}] = [")
    out += [f"    {rlit(nm)}," for nm in feat_names]
    out.append("];")
    out.append("#[rustfmt::skip]")
    out.append(f"pub static NAME_TO_ID: [(&str, u32); {n}] = [")
    out += [f"    ({rlit(classes[i]['name'])}, {i})," for i in order]
    out.append("];")
    out.append(
        "\n#[derive(Clone, Copy, Debug)]\n"
        "pub struct EClassMeta {\n"
        "    pub name: &'static str,\n"
        "    pub abstract_: bool,\n"
        "    pub sups: &'static [u32],\n"
        "    pub own: &'static [u16],\n"
        "}\n"
    )
    out.append("#[rustfmt::skip]")
    out.append(f"pub static ECLASS: [EClassMeta; {n}] = [")
    for i, c in enumerate(classes):
        sups = "&[" + ",".join(str(s) for s in sup_idx[i]) + "]"
        own = "&[" + ",".join(str(feat_ids[f]) for f in c["own"]) + "]"
        out.append(
            f"    EClassMeta {{ name: {rlit(c['name'])}, abstract_: "
            f"{'true' if c['abstract'] else 'false'}, sups: {sups}, own: {own} }},"
        )
    out.append("];")
    out.append(
        '\npub fn name_to_id(name: &str) -> Option<u32> {\n'
        "    NAME_TO_ID.binary_search_by(|(n, _)| n.cmp(name))\n"
        "        .ok().map(|i| NAME_TO_ID[i].1)\n"
        "}\n"
    )
    return "\n".join(out)


if __name__ == "__main__":
    src = sys.argv[1]
    text = gen(src)
    if len(sys.argv) > 2:
        with open(sys.argv[2], "w") as f:
            f.write(text)
        print("wrote", sys.argv[2], "| classes:", text.count("EClassMeta {"))
    else:
        sys.stdout.write(text)