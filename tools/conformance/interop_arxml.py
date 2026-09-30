#!/usr/bin/env python3
"""arxml 写路径 round-trip 校验（真实 AUTOSAR 样本）。

对 artop-cpp `output/samples/` 下的每份非空真实样本：
    A. load -> save 必须成功（期望 stdout 含 `ROUNDTRIP-OK`）；
    B. 写路径幂等：对再一次 load -> save 的输出必须与第一次**逐字节相同**；
    C. load -> save 的输出必须与原文**逐字节相同**（Rust saver 已复刻 C++/Java
       的 arxml 版式，含 XML 属性顺序）；若不同，则进一步用规范化比较区分
       「只是版式差异」还是「结构真的不同」，以便定位回归。

样本来自 artop-cpp（CI 中 checkout 到 reference/artop-cpp），因此这条用例同时
证明了 Rust 能读 C++ 生态产出的真实 arxml、并原样写回。

用法:
  python3 tools/conformance/interop_arxml.py \
      --rust target/debug/examples/arxml_roundtrip \
      --samples reference/artop-cpp/output/samples
退出码: 0(全部通过); 1(存在失败)。
"""
import argparse
import os
import subprocess
import sys
import tempfile
from xml.dom import minidom

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))


def resolve(path):
    return path if os.path.isabs(path) else os.path.join(ROOT, path)


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)


def canon(node, out):
    """Canonical form: elements/children in document order, attributes sorted by
    name, whitespace-only text dropped (indentation is layout, not content)."""
    t = node.nodeType
    if t == node.DOCUMENT_NODE:
        for c in node.childNodes:
            canon(c, out)
    elif t == node.ELEMENT_NODE:
        out.append("<" + node.tagName)
        for name, val in sorted((a.name, a.value) for a in node.attributes.values()):
            out.append("\x01" + name + "\x02" + val)
        out.append(">")
        for c in node.childNodes:
            canon(c, out)
        out.append("</" + node.tagName + ">")
    elif t == node.COMMENT_NODE:
        out.append("<!--" + node.data + "-->")
    elif t in (node.TEXT_NODE, node.CDATA_SECTION_NODE):
        text = " ".join(node.data.split())
        if text:
            out.append("\x03" + text)


def canonical_xml(path):
    with open(path, encoding="utf-8") as f:
        dom = minidom.parseString(f.read())
    out = []
    canon(dom, out)
    return "".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rust", required=True, help="Rust arxml_roundtrip example binary")
    ap.add_argument("--samples", required=True, help="directory of *.arxml samples")
    args = ap.parse_args()

    rust = resolve(args.rust)
    samples = resolve(args.samples)
    if not os.path.exists(rust):
        sys.exit(f"Rust harness missing: {rust}")
    if not os.path.isdir(samples):
        sys.exit(f"samples dir missing: {samples}")

    failures = []
    checked = 0
    with tempfile.TemporaryDirectory(prefix="arxml-interop-") as tmp:
        for name in sorted(os.listdir(samples)):
            if not name.endswith(".arxml"):
                continue
            src = os.path.join(samples, name)
            if os.path.getsize(src) == 0:
                print(f"[SKIP] {name} (empty placeholder)")
                continue
            checked += 1
            out1 = os.path.join(tmp, name + ".1")
            out2 = os.path.join(tmp, name + ".2")

            def step(label, cmd, expect):
                r = run(cmd)
                ok = r.returncode == 0 and expect in r.stdout
                print(f"[{'PASS' if ok else 'FAIL'}] {name}: {label}")
                if not ok:
                    print(f"       cmd: {' '.join(cmd)}")
                    print(f"       rc={r.returncode}")
                    if r.stdout.strip():
                        print(f"       stdout: {r.stdout.strip()}")
                    if r.stderr.strip():
                        print(f"       stderr: {r.stderr.strip()}")
                    failures.append(f"{name}: {label}")
                return ok

            # A/B: load -> save, then load -> save again (idempotency).
            if not step("load -> save", [rust, "roundtrip", src, out1], "ROUNDTRIP-OK"):
                continue
            if not step("re-load -> save", [rust, "roundtrip", out1, out2], "ROUNDTRIP-OK"):
                continue

            with open(out1, encoding="utf-8") as f:
                a = f.read()
            with open(out2, encoding="utf-8") as f:
                b = f.read()
            if a != b:
                print(f"[FAIL] {name}: write path not idempotent")
                failures.append(f"{name}: idempotency")
            else:
                print(f"[PASS] {name}: write path idempotent")

            # C: byte-for-byte equality with the original. The Rust saver now
            # reproduces the C++/Java arxml layout exactly (including XML
            # attribute order), so a real round-trip must be byte-identical.
            with open(src, "rb") as f:
                src_bytes = f.read()
            with open(out1, "rb") as f:
                out_bytes = f.read()
            if src_bytes == out_bytes:
                print(f"[PASS] {name}: byte-identical to original")
            else:
                # Distinguish a cosmetic regression from a semantic one.
                try:
                    structural = canonical_xml(src) == canonical_xml(out1)
                except Exception:  # noqa: BLE001 - diagnostic only
                    structural = False
                kind = "structurally equal, layout differs" if structural else "STRUCTURALLY DIFFERENT"
                print(f"[FAIL] {name}: output != original ({kind})")
                failures.append(f"{name}: byte-identical ({kind})")

    print()
    if failures:
        print(f"arxml interop: {len(failures)} failure(s) over {checked} sample(s)")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"arxml interop: {checked} sample(s) OK (load/save + idempotent + byte-identical)")
    return 0


if __name__ == "__main__":
    sys.exit(main())