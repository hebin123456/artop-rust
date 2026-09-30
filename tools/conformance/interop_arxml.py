#!/usr/bin/env python3
"""arxml 读写互操作校验（真实 AUTOSAR 样本）。

分两部分：

一、Rust 写路径（始终执行）—— 对 artop-cpp `output/samples/` 下的每份非空真实样本：
    A. load -> save 必须成功（期望 stdout 含 `ROUNDTRIP-OK`）；
    B. 写路径幂等：对再一次 load -> save 的输出必须与第一次**逐字节相同**；
    C. load -> save 的输出必须与原文**逐字节相同**（Rust saver 已复刻 C++/Java
       的 arxml 版式，含 XML 属性顺序）；若不同，则进一步用规范化比较区分
       「只是版式差异」还是「结构真的不同」，以便定位回归。

二、Rust <-> C++ 双向交接（给出 `--cpp` 时执行）—— 四步 A/B/C/D：
    A. Rust 读样本写回 -> C++ 读回校验；
    B. C++ 读样本写回   -> Rust 读回校验；
    C. Rust 读 C++ 的写回结果再写回 -> C++ 读回校验；
    D. C++ 读 Rust 的写回结果再写回 -> Rust 读回校验。
    两侧 harness：`tools/conformance/interop_arxml_main.cpp` 与
    `crates/emf-artop/artop-runtime/examples/arxml_roundtrip.rs`。
    C++ 侧的 `check` 断言「能读成 AUTOSAR 根 + 至少一个 AR-PACKAGE」——因为
    artop-cpp 的 artop-runtime 默认只带最小 autosar40.ecore（见 harness 注释），
    其 `roundtrip` 不保证逐字节。Rust 侧的 `check` 断言根类为 AUTOSAR。

样本来自 artop-cpp（CI 中 checkout 到 reference/artop-cpp），因此这条用例同时
证明了 Rust 能读 C++ 生态产出的真实 arxml、并原样写回，以及两侧能互相读写。

用法:
  python3 tools/conformance/interop_arxml.py \
      --rust target/debug/examples/arxml_roundtrip \
      --samples reference/artop-cpp/output/samples
  # 追加 Rust <-> C++ 双向：
  python3 tools/conformance/interop_arxml.py \
      --rust target/debug/examples/arxml_roundtrip \
      --cpp  tools/conformance/build/interop_arxml \
      --samples reference/artop-cpp/output/samples \
      --ecore448 reference/artop-cpp/models/autosar448/autosar448.ecore \
      --ecore-gautosar reference/artop-cpp/models/gautosar/gautosar.ecore
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


def run(cmd, env=None):
    return subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT, env=env)


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
    ap.add_argument("--cpp", help="C++ interop_arxml harness binary (enables Rust<->C++)")
    ap.add_argument("--samples", required=True, help="directory of *.arxml samples")
    ap.add_argument("--ecore448", help="AUTOSAR448 .ecore for the C++ dynamic metamodel")
    ap.add_argument("--ecore-gautosar", help="gautosar .ecore for the C++ dynamic metamodel")
    args = ap.parse_args()

    rust = resolve(args.rust)
    samples = resolve(args.samples)
    cpp = resolve(args.cpp) if args.cpp else None
    if not os.path.exists(rust):
        sys.exit(f"Rust harness missing: {rust}")
    if not os.path.isdir(samples):
        sys.exit(f"samples dir missing: {samples}")
    if cpp and not os.path.exists(cpp):
        sys.exit(f"C++ harness missing: {cpp}")

    # The C++ harness reads the AUTOSAR metamodel from these env vars (falls back
    # to its built-in minimal autosar40.ecore when unset).
    cpp_env = dict(os.environ)
    if args.ecore448:
        cpp_env["ARXML_INTEROP_ECORE_448"] = resolve(args.ecore448)
    if args.ecore_gautosar:
        cpp_env["ARXML_INTEROP_ECORE_GAUTOSAR"] = resolve(args.ecore_gautosar)

    failures = []

    def step(name, cmd, expect, env=None):
        r = run(cmd, env=env)
        ok = r.returncode == 0 and expect in r.stdout
        print(f"[{'PASS' if ok else 'FAIL'}] {name}")
        if not ok:
            print(f"       cmd: {' '.join(cmd)}")
            print(f"       rc={r.returncode}")
            if r.stdout.strip():
                print(f"       stdout: {r.stdout.strip()}")
            if r.stderr.strip():
                print(f"       stderr: {r.stderr.strip()}")
            failures.append(name)
        return ok

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
            out1 = os.path.join(tmp, name + ".rust1")
            out2 = os.path.join(tmp, name + ".rust2")

            # --- 一、Rust 写路径 ---
            if not step(f"{name}: rust load -> save",
                        [rust, "roundtrip", src, out1], "ROUNDTRIP-OK"):
                continue
            if not step(f"{name}: rust re-load -> save",
                        [rust, "roundtrip", out1, out2], "ROUNDTRIP-OK"):
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

            # byte-for-byte equality with the original (see module docstring).
            with open(src, "rb") as f:
                src_bytes = f.read()
            with open(out1, "rb") as f:
                out_bytes = f.read()
            if src_bytes == out_bytes:
                print(f"[PASS] {name}: byte-identical to original")
            else:
                try:
                    structural = canonical_xml(src) == canonical_xml(out1)
                except Exception:  # noqa: BLE001 - diagnostic only
                    structural = False
                kind = "structurally equal, layout differs" if structural else "STRUCTURALLY DIFFERENT"
                print(f"[FAIL] {name}: output != original ({kind})")
                failures.append(f"{name}: byte-identical ({kind})")

            # --- 二、Rust <-> C++ 双向交接 ---
            if not cpp:
                continue
            cpp_out = os.path.join(tmp, name + ".cpp1")
            rust_echo = os.path.join(tmp, name + ".rust3")
            cpp_echo = os.path.join(tmp, name + ".cpp2")

            # A. Rust 写 -> C++ 读
            step(f"{name}: A cpp check rust output",
                 [cpp, "check", out1], "CHECK-OK", env=cpp_env)
            # B. C++ 写 -> Rust 读
            if step(f"{name}: B cpp load -> save",
                    [cpp, "roundtrip", src, cpp_out], "ROUNDTRIP-OK", env=cpp_env):
                step(f"{name}: B rust check cpp output",
                     [rust, "check", cpp_out], "CHECK-OK")
            # C. Rust 读 C++ 的写回再写回 -> C++ 读
            if os.path.exists(cpp_out):
                if step(f"{name}: C rust roundtrip cpp output",
                        [rust, "roundtrip", cpp_out, rust_echo], "ROUNDTRIP-OK"):
                    step(f"{name}: C cpp check rust re-emit",
                         [cpp, "check", rust_echo], "CHECK-OK", env=cpp_env)
            # D. C++ 读 Rust 的写回再写回 -> Rust 读
            if step(f"{name}: D cpp roundtrip rust output",
                    [cpp, "roundtrip", out1, cpp_echo], "ROUNDTRIP-OK", env=cpp_env):
                step(f"{name}: D rust check cpp re-emit",
                     [rust, "check", cpp_echo], "CHECK-OK")

    print()
    if failures:
        print(f"arxml interop: {len(failures)} failure(s) over {checked} sample(s)")
        for f in failures:
            print(f"  - {f}")
        return 1
    scope = "load/save + idempotent + byte-identical"
    if cpp:
        scope += " + Rust<->C++ bidirectional handoff"
    print(f"arxml interop: {checked} sample(s) OK ({scope})")
    return 0


if __name__ == "__main__":
    sys.exit(main())